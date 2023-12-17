import { useEffect, useState } from "react";
import { fromBlob, fromUrl } from "geotiff";
import { Matrix4 } from "@math.gl/core";

import {
  getChannelStats,
  loadBioformatsZarr,
  loadMultiTiff,
  loadOmeTiff,
  loadOmeZarr,
} from "@hms-dbmi/viv";
import { bin } from "d3-array";

import { GLOBAL_SLIDER_DIMENSION_FIELDS } from "./constants";
import * as psudoAnalysis from "psudo-analysis";
import _ from "lodash";

const MAX_CHANNELS_FOR_SNACKBAR_WARNING = 40;

/**
 * Guesses whether string URL or File is for an OME-TIFF image.
 * @param {string | File} urlOrFile
 */
function isOMETIFF(urlOrFile) {
  if (Array.isArray(urlOrFile)) return false; // local Zarr is array of File Objects
  const name = typeof urlOrFile === "string" ? urlOrFile : urlOrFile.name;
  return name.includes("ome.tiff") || name.includes("ome.tif");
}

function isMultiTiff(urlOrFile) {
  const filenames = Array.isArray(urlOrFile)
    ? urlOrFile.map((f) => f.name)
    : urlOrFile.split(",");
  for (const filename of filenames) {
    const lowerCaseName = filename.toLowerCase();
    if (!(lowerCaseName.includes(".tiff") || lowerCaseName.includes(".tif")))
      return false;
  }
  return true;
}

class UnsupportedBrowserError extends Error {
  constructor(message) {
    super(message);
    this.name = "UnsupportedBrowserError";
  }
}

/**
 *
 * @param {string | File} src
 * @param {import('../../src/loaders/omexml').OMEXML} rootMeta
 * @param {number} levels
 * @param {import('../../src/loaders/tiff/pixel-source').TiffPixelSource[]} data
 */
async function getTotalImageCount(src, rootMeta, data) {
  const from = typeof src === "string" ? fromUrl : fromBlob;
  const tiff = await from(src);
  const firstImage = await tiff.getImage(0);
  const hasSubIFDs = Boolean(firstImage?.fileDirectory?.SubIFDs);
  if (hasSubIFDs) {
    return rootMeta.reduce((sum, imgMeta) => {
      const {
        Pixels: { SizeC, SizeT, SizeZ },
      } = imgMeta;
      const numImagesPerResolution = SizeC * SizeT * SizeZ;
      return numImagesPerResolution + sum;
    }, 1);
  }
  const levels = data[0].length;
  const {
    Pixels: { SizeC, SizeT, SizeZ },
  } = rootMeta[0];
  const numImagesPerResolution = SizeC * SizeT * SizeZ;
  return numImagesPerResolution * levels;
}

/**
 * Given an image source, creates a PixelSource[] and returns XML-meta
 *
 * @param {string | File | File[]} urlOrFile
 * @param {} handleOffsetsNotFound
 * @param {*} handleLoaderError
 */
export async function createLoader(
  urlOrFile,
  handleOffsetsNotFound,
  handleLoaderError
) {
  // If the loader fails to load, handle the error (show an error snackbar).
  // Otherwise load.
  try {
    // OME-TIFF
    if (isOMETIFF(urlOrFile)) {
      if (urlOrFile instanceof File) {
        // TODO(2021-05-09): temporarily disable `pool` until inline worker module is fixed.
        const source = await loadOmeTiff(urlOrFile, {
          images: "all",
          pool: false,
        });
        return source;
      }
      const url = urlOrFile;
      const res = await fetch(url.replace(/ome\.tif(f?)/gi, "offsets.json"));
      const isOffsetsNot200 = res.status !== 200;
      const offsets = !isOffsetsNot200 ? await res.json() : undefined;
      // TODO(2021-05-06): temporarily disable `pool` until inline worker module is fixed.
      const source = await loadOmeTiff(urlOrFile, {
        offsets,
        images: "all",
        pool: false,
      });

      // Show a warning if the total number of channels/images exceeds a fixed amount.
      // Non-Bioformats6 pyramids use Image tags for pyramid levels and do not have offsets
      // built in to the format for them, hence the ternary.
      const totalImageCount = await getTotalImageCount(
        urlOrFile,
        source.map((s) => s.metadata),
        source.map((s) => s.data)
      );
      if (
        isOffsetsNot200 &&
        totalImageCount > MAX_CHANNELS_FOR_SNACKBAR_WARNING
      ) {
        handleOffsetsNotFound(true);
      }
      return source;
    }

    if (
      Array.isArray(urlOrFile) &&
      typeof urlOrFile[0].arrayBuffer !== "function"
    ) {
      throw new UnsupportedBrowserError(
        "Cannot upload a local Zarr or flat TIFF files with this browser. Try using Chrome, Firefox, or Microsoft Edge."
      );
    }

    // Multiple flat tiffs
    if (isMultiTiff(urlOrFile)) {
      const multiTiffFiles = Array.isArray(urlOrFile)
        ? urlOrFile
        : urlOrFile.split(",");
      const mutiTiffSources = multiTiffFiles.map((e, i) => [
        { c: i, z: 0, t: 0 },
        e,
      ]);
      const source = await loadMultiTiff(mutiTiffSources, {
        images: "all",
        pool: false,
      });
      return source;
    }

    // Bio-Formats Zarr
    let source;
    try {
      source = await loadBioformatsZarr(urlOrFile);
    } catch {
      // try ome-zarr
      const res = await loadOmeZarr(urlOrFile, { type: "multiscales" });
      // extract metadata into OME-XML-like form
      const metadata = {
        Pixels: {
          Channels: res.metadata.omero.channels.map((c) => ({
            Name: c.label,
            SamplesPerPixel: 1,
          })),
        },
      };
      source = { data: res.data, metadata };
    }
    return source;
  } catch (e) {
    if (e instanceof UnsupportedBrowserError) {
      handleLoaderError(e.message);
    } else {
      console.error(e); // eslint-disable-line
      handleLoaderError(null);
    }
    return { data: null };
  }
}

// Get the last part of a url (minus query parameters) to be used
// as a display name for avivator.
export function getNameFromUrl(url) {
  return url.split("?")[0].split("/").slice(-1)[0];
}

/**
 * Return the midpoint of the global dimensions as a default selection.
 *
 * @param { import('../../src/types').PixelSource<['t', 'z', 'c']> } pixelSource
 */
function getDefaultGlobalSelection({ labels, shape }) {
  const dims = labels
    .map((name, i) => [name, i])
    .filter((d) => GLOBAL_SLIDER_DIMENSION_FIELDS.includes(d[0]));

  /**
   * @type { { t: number, z: number, c: number  } }
   */
  const selection = {};
  dims.forEach(([name, index]) => {
    selection[name] = Math.floor((shape[index] || 0) / 2);
  });

  return selection;
}

/**
 * @param {Array.<number>} shape loader shape
 */
export function isInterleaved(shape) {
  const lastDimSize = shape[shape.length - 1];
  return lastDimSize === 3 || lastDimSize === 4;
}

// Create a default selection using the midpoint of the available global dimensions,
// and then the first four available selections from the first selectable channel.
/**
 *
 * @param { import('../../src/types').PixelSource<['t', 'z', 'c']> } pixelSource
 */
export function buildDefaultSelection(pixelSource) {
  const numChannels = pixelSource.shape[pixelSource.shape.length - 3];
  let selection = [];
  const globalSelection = getDefaultGlobalSelection(pixelSource);
  // First non-global dimension with some sort of selectable values.

  const firstNonGlobalDimension = pixelSource.labels
    .map((name, i) => ({ name, size: pixelSource.shape[i] }))
    .find((d) => !GLOBAL_SLIDER_DIMENSION_FIELDS.includes(d.name) && d.size);


  for (let i = 0; i < Math.min(6, firstNonGlobalDimension.size); i += 1) {
    selection.push({
      [firstNonGlobalDimension.name]: i,
      ...globalSelection,
    });
  }

  selection = isInterleaved(pixelSource.shape)
    ? [{ ...selection[0], c: 0 }]
    : selection;
  return selection;
}

export function hexToRgb(hex) {
  // https://stackoverflow.com/questions/5623838/rgb-to-hex-and-hex-to-rgb
  const result = /^#?([a-f\d]{2})([a-f\d]{2})([a-f\d]{2})$/i.exec(hex);
  return result.map((d) => parseInt(d, 16)).slice(1);
}

export function range(length) {
  return [...Array(length).keys()];
}

export function useWindowSize(scaleWidth = 1, scaleHeight = 1, elem = null) {
  function getSize() {
    return {
      width: window.innerWidth * scaleWidth,
      height: window.innerHeight * scaleHeight,
    };
  }

  const [windowSize, setWindowSize] = useState(getSize());
  useEffect(() => {
    const handleResize = () => {
      setWindowSize(getSize());
    };
    window.addEventListener("resize", handleResize);
    return () => {
      window.removeEventListener("resize", handleResize);
    };
  });
  return windowSize;
}
export async function getChannelPayload(
  channelsVisible,
  colors,
  selections,
  contrastLimits,
  loader,
  pyramidResolution,
  sampleSize = null,
  lens = false
) {
  const channelsPayload = [];
  let indices = null;

  await Promise.all(
    channelsVisible.map(async (d, i) => {
      if (d) {
        const raster = await loader?.[pyramidResolution]?.getRaster({
          selection: selections[i],
        });
        let rasterArray = raster.data;
        if (sampleSize != null && indices == null) {
          indices = _.times(sampleSize, () =>
            _.random(0, rasterArray.length - 1)
          );
        }
        if (sampleSize != null) {
          rasterArray = new Uint16Array(sampleSize);
          indices.forEach((d, ii) => {
            rasterArray[ii] = raster.data[d];
          });
        }
        const channelPayload = {
          color: colors[i],
          contrastLimits: contrastLimits[i],
          selection: selections[i],
          data: rasterArray,
        };
        channelsPayload.push(channelPayload);
      }
    })
  );
  return channelsPayload;
}

export function createContiguousArrays(channelList) {
  let dataLength = channelList[0]?.data?.length || 0;
  let intensityArray = new Uint16Array(dataLength * channelList.length);
  let colorArray = new Uint16Array(3 * channelList.length);
  let contrastLimitsArray = new Uint16Array(2 * channelList.length);
  channelList.forEach((d, i) => {
    intensityArray.set(d.data, i * dataLength);
    colorArray.set(d.color, i * 3);
    contrastLimitsArray.set(d.contrastLimits, i * 2);
  });
  return { intensityArray, colorArray, contrastLimitsArray };
}

export async function calculateLensPaletteLoss(channelsPayload, luminanceValue) {
  const { intensityArray, colorArray, contrastLimitsArray } =
    createContiguousArrays(channelsPayload);
  if (intensityArray.length == 0) return null;
  let paletteCost = psudoAnalysis.calculate_palette_loss(
    intensityArray,
    colorArray,
    contrastLimitsArray,
    luminanceValue
  );
  return paletteCost;
}

export async function calculatePaletteLoss(
  channelsVisible,
  loader,
  selections,
  contrastLimits,
  colors,
  pyramidResolution,
  luminanceValue
) {
  const channelsPayload = await getChannelPayload(
    channelsVisible,
    colors,
    selections,
    contrastLimits,
    loader,
    pyramidResolution,
    10000
  );
  const { intensityArray, colorArray, contrastLimitsArray } =
    createContiguousArrays(channelsPayload);
  let paletteCost = psudoAnalysis.calculate_palette_loss(
    intensityArray,
    colorArray,
    contrastLimitsArray,
    luminanceValue
  );
  return paletteCost;
}

export async function calculateConfusionLoss(
  channelsVisible,
  loader,
  selections,
  contrastLimits,
  colors,
  pyramidResolution
) {
  const subsampleSize = 10000;
  let sample_indices = null;
  const channelsPayload = await getChannelPayload(
    channelsVisible,
    colors,
    selections,
    contrastLimits,
    loader,
    pyramidResolution,
    subsampleSize
  );
  let colorList = [];
  // create a uint16array of length len(channelsPayload) * subsample_size
  const rasterArray = new Uint16Array(subsampleSize * channelsPayload.length);

  //   channelsPayload.map(async (d, i) => {
  //     colorList.push(...d.color);
  //     // const resolution = pyramidResolution;
  //     const raster = await loader?.[pyramidResolution]?.getRaster({
  //       selection: d.selection,
  //     });
  //     // If sample_indices is null, then create a subsample of the raster
  //     if (sample_indices == null)
  //       sample_indices = _.times(subsample_size, () =>
  //         _.random(0, raster.data.length)
  //       );
  //     // Write the data at these indices to the rasterArray
  //     sample_indices.forEach((d, ii) => {
  //       rasterArray[ii + i * subsample_size] = raster.data[d];
  //     });

  //     console.log("raster", rasterArray);
  //     // raster size
  //   })
  // );
}

export async function getSingleSelectionStats2D(
  { loader, selection, pyramidResolution, channelVisible },
  computeStats = false
) {
  const data = Array.isArray(loader) ? loader[loader.length - 1] : loader;
  const raster = await data.getRaster({ selection });
  if (computeStats) {
    const selectionStats = getChannelStats(raster.data);
    const { domain, contrastLimits } = selectionStats;
    return { domain, contrastLimits };
  } else {
    const contrastLimits = channelVisible ? await getGMMContrastLimits({
      loader,
      selection,
      pyramidResolution,
    }) : [0, 65535]
    // In the future do some logic to determine the dtype and return accordingly\
    return { domain: [0, 65535], contrastLimits: contrastLimits };
  }
}

export async function getGMMContrastLimits({
  loader,
  selection,
  pyramidResolution,
}) {
  try {
    let raster = await loader?.[pyramidResolution]?.getRaster({
      selection: selection,
    });
    const contrastLimits = psudoAnalysis.channel_gmm(raster.data);
    const intContrastLimits = [
      _.toInteger(contrastLimits[0]),
      _.toInteger(contrastLimits[1]),
    ];
    delete raster.data;
    return intContrastLimits;
  } catch (e) {
    return [0, 65535];
  }
}

export function getChannelGraphData({ data, color, selection }) {
  const numBins = 50;
  let channelData = {};
  channelData["data"] = data;
  channelData["logData"] = psudoAnalysis.ln(data);
  // Number of bins you want
  const binF = bin()
    .domain([0, Math.log(65535)]) // Setting the range of your data
    .thresholds(numBins);
  const binnedData = binF(channelData.logData);
  // Get the frequencies as fractions
  const frequencies = binnedData.map((d, i) => [
    i,
    d.length / channelData.logData.length,
  ]);
  channelData["frequencies"] = frequencies;
  channelData["color"] = color;
  channelData["selection"] = selection;
  return channelData;
}

export function getGraphData(
  channelData,
  colors,
  lensSelection,
  channelsVisible,
  selections
) {
  let graphData = [];
  if (_.every(lensSelection, (num) => num === 0)) {
    (channelsVisible || []).forEach((d, i) => {
      if (d == true) {
        const selection = selections[i];
        let thisChannelsData = channelData.filter((d) => {
          return _.isEqual(d.selection, selection);
        })[0];
        const color = colors[i];
        let thisChannelsGraphData = getChannelGraphData({
          ...thisChannelsData,
          color: color,
        });
        graphData.push(thisChannelsGraphData);
      }
    });
  } else {
    lensSelection.forEach((d, i) => {
      if (d == 1) {
        const selection = selections[i];
        let thisChannelsData = channelData.filter((d) => {
          return _.isEqual(d.selection, selection);
        })[0];
        const color = colors[i];
        let thisChannelsGraphData = getChannelGraphData({
          ...thisChannelsData,
          color: color,
        });
        graphData.push(thisChannelsGraphData);
      }
    });
  }
  return graphData;
}

export const getSingleSelectionStats = async ({
  loader,
  selection,
  pyramidResolution,
  channelVisible
}) => {
  const getStats = getSingleSelectionStats2D;
  return getStats({ loader, selection, pyramidResolution, channelVisible });
};

export const getMultiSelectionStats = async ({
  loader,
  selections,
  pyramidResolution,
  channelsVisible
}) => {
  const stats = await Promise.all(
    selections.map((selection, i) =>
      getSingleSelectionStats({ loader, selection, pyramidResolution, channelVisible: channelsVisible[i] })
    )
  );
  const domains = stats.map((stat) => stat.domain);
  const contrastLimits = stats.map((stat) => stat.contrastLimits);
  return { domains, contrastLimits };
};

/* eslint-disable no-useless-escape */

// https://stackoverflow.com/a/11381730
export function isMobileOrTablet() {
  let check = false;
  // eslint-disable-next-line func-names
  (function (a) {
    if (
      /(android|bb\d+|meego).+mobile|avantgo|bada\/|blackberry|blazer|compal|elaine|fennec|hiptop|iemobile|ip(hone|od)|iris|kindle|lge |maemo|midp|mmp|mobile.+firefox|netfront|opera m(ob|in)i|palm( os)?|phone|p(ixi|re)\/|plucker|pocket|psp|series(4|6)0|symbian|treo|up\.(browser|link)|vodafone|wap|windows ce|xda|xiino|android|ipad|playbook|silk/i.test(
        a
      ) ||
      /1207|6310|6590|3gso|4thp|50[1-6]i|770s|802s|a wa|abac|ac(er|oo|s\-)|ai(ko|rn)|al(av|ca|co)|amoi|an(ex|ny|yw)|aptu|ar(ch|go)|as(te|us)|attw|au(di|\-m|r |s )|avan|be(ck|ll|nq)|bi(lb|rd)|bl(ac|az)|br(e|v)w|bumb|bw\-(n|u)|c55\/|capi|ccwa|cdm\-|cell|chtm|cldc|cmd\-|co(mp|nd)|craw|da(it|ll|ng)|dbte|dc\-s|devi|dica|dmob|do(c|p)o|ds(12|\-d)|el(49|ai)|em(l2|ul)|er(ic|k0)|esl8|ez([4-7]0|os|wa|ze)|fetc|fly(\-|_)|g1 u|g560|gene|gf\-5|g\-mo|go(\.w|od)|gr(ad|un)|haie|hcit|hd\-(m|p|t)|hei\-|hi(pt|ta)|hp( i|ip)|hs\-c|ht(c(\-| |_|a|g|p|s|t)|tp)|hu(aw|tc)|i\-(20|go|ma)|i230|iac( |\-|\/)|ibro|idea|ig01|ikom|im1k|inno|ipaq|iris|ja(t|v)a|jbro|jemu|jigs|kddi|keji|kgt( |\/)|klon|kpt |kwc\-|kyo(c|k)|le(no|xi)|lg( g|\/(k|l|u)|50|54|\-[a-w])|libw|lynx|m1\-w|m3ga|m50\/|ma(te|ui|xo)|mc(01|21|ca)|m\-cr|me(rc|ri)|mi(o8|oa|ts)|mmef|mo(01|02|bi|de|do|t(\-| |o|v)|zz)|mt(50|p1|v )|mwbp|mywa|n10[0-2]|n20[2-3]|n30(0|2)|n50(0|2|5)|n7(0(0|1)|10)|ne((c|m)\-|on|tf|wf|wg|wt)|nok(6|i)|nzph|o2im|op(ti|wv)|oran|owg1|p800|pan(a|d|t)|pdxg|pg(13|\-([1-8]|c))|phil|pire|pl(ay|uc)|pn\-2|po(ck|rt|se)|prox|psio|pt\-g|qa\-a|qc(07|12|21|32|60|\-[2-7]|i\-)|qtek|r380|r600|raks|rim9|ro(ve|zo)|s55\/|sa(ge|ma|mm|ms|ny|va)|sc(01|h\-|oo|p\-)|sdk\/|se(c(\-|0|1)|47|mc|nd|ri)|sgh\-|shar|sie(\-|m)|sk\-0|sl(45|id)|sm(al|ar|b3|it|t5)|so(ft|ny)|sp(01|h\-|v\-|v )|sy(01|mb)|t2(18|50)|t6(00|10|18)|ta(gt|lk)|tcl\-|tdg\-|tel(i|m)|tim\-|t\-mo|to(pl|sh)|ts(70|m\-|m3|m5)|tx\-9|up(\.b|g1|si)|utst|v400|v750|veri|vi(rg|te)|vk(40|5[0-3]|\-v)|vm40|voda|vulc|vx(52|53|60|61|70|80|81|83|85|98)|w3c(\-| )|webc|whit|wi(g |nc|nw)|wmlb|wonu|x700|yas\-|your|zeto|zte\-/i.test(
        a.substr(0, 4)
      )
    )
      check = true;
  })(navigator.userAgent || navigator.vendor || window.opera);
  return check;
}

/* eslint-disable no-useless-escape */

/**
 * @param { import('../../src/loaders/omexml').OMEXML[0] } imgMeta
 */
export function guessRgb({ Pixels }) {
  const numChannels = Pixels.Channels.length;
  const { SamplesPerPixel } = Pixels.Channels[0];

  const is3Channel8Bit = numChannels === 3 && Pixels.Type === "uint8";
  const interleavedRgb =
    Pixels.SizeC === 3 && numChannels === 1 && Pixels.Interleaved;

  return SamplesPerPixel === 3 || is3Channel8Bit || interleavedRgb;
}

export function truncateDecimalNumber(value, maxLength) {
  if (!value && value !== 0) return "";
  const stringValue = value.toString();
  return stringValue.length > maxLength
    ? stringValue.substring(0, maxLength).replace(/\.$/, "")
    : stringValue;
}

/**
 * Get physical size scaling Matrix4
 * @param {Object} loader PixelSource
 */
export function getPhysicalSizeScalingMatrix(loader) {
  const { x, y, z } = loader?.meta?.physicalSizes ?? {};
  if (x?.size && y?.size && z?.size) {
    const min = Math.min(z.size, x.size, y.size);
    const ratio = [x.size / min, y.size / min, z.size / min];
    return new Matrix4().scale(ratio);
  }
  return new Matrix4().identity();
}

export const getSquare = (centerX, centerY, width, height) => {
  const data = [
    [
      [centerX - width / 2, centerY - height / 2],
      [centerX - width / 2, centerY + height / 2],
    ],
    [
      [centerX - width / 2, centerY + height / 2],
      [centerX + width / 2, centerY + height / 2],
    ],
    [
      [centerX + width / 2, centerY + height / 2],
      [centerX + width / 2, centerY - height / 2],
    ],
    [
      [centerX + width / 2, centerY - height / 2],
      [centerX - width / 2, centerY - height / 2],
    ],
  ];
  return data;
};

export async function getLensIntensityValues(
  coordinate,
  viewState,
  loader,
  pyramidResolution,
  lensRadius,
  channelsVisible,
  selections,
  setMovingLens,
  contrastLimits,
  colors
) {
  let multiplier = 1 / Math.pow(2, viewState.zoom);
  const size = lensRadius * multiplier;
  const sizeAtPyramidLevel = size / Math.pow(2, pyramidResolution);
  const x = coordinate[0] / Math.pow(2, pyramidResolution);
  const y = coordinate[1] / Math.pow(2, pyramidResolution);
  const x_center = x;
  const y_center = y;

  const loaderAtThisLevel = loader[pyramidResolution];
  const shapeAtThisLevel = loaderAtThisLevel.shape;
  const tileSizeAtThisLevel = [
    loaderAtThisLevel.tileSize < shapeAtThisLevel[shapeAtThisLevel.length - 1]
      ? loaderAtThisLevel.tileSize
      : shapeAtThisLevel[shapeAtThisLevel.length - 1],
    loaderAtThisLevel.tileSize < shapeAtThisLevel[shapeAtThisLevel.length - 2]
      ? loaderAtThisLevel.tileSize
      : shapeAtThisLevel[shapeAtThisLevel.length - 2],
  ];
  // Shape is an array that ends with y,x
  const xAtThisLevel = shapeAtThisLevel[shapeAtThisLevel.length - 1];
  const yAtThisLevel = shapeAtThisLevel[shapeAtThisLevel.length - 2];
  // Shape ends with y,x, make sure the follwing do not exceed the shape
  const xMin = Math.floor(
    x - sizeAtPyramidLevel < 0 ? 0 : x - sizeAtPyramidLevel
  );
  const xMax = Math.ceil(
    x + sizeAtPyramidLevel > xAtThisLevel
      ? xAtThisLevel
      : x + sizeAtPyramidLevel
  );
  const yMin = Math.floor(
    y - sizeAtPyramidLevel < 0 ? 0 : y - sizeAtPyramidLevel
  );
  const yMax = Math.ceil(
    y + sizeAtPyramidLevel > yAtThisLevel
      ? yAtThisLevel
      : y + sizeAtPyramidLevel
  );
  // get viewport
  // Calculate the indices of the tiles
  const xMinIndex = Math.floor(xMin / tileSizeAtThisLevel[0]);
  const xMaxIndex = Math.ceil(xMax / tileSizeAtThisLevel[0]) - 1; // we subtract 1 because we want the index of the last tile, not the count
  const yMinIndex = Math.floor(yMin / tileSizeAtThisLevel[1]);
  const yMaxIndex = Math.ceil(yMax / tileSizeAtThisLevel[1]) - 1;

  // Ensure the indices do not exceed the number of tiles in each dimension
  const xMinClamped = Math.max(xMinIndex, 0);
  const xMaxClamped = Math.min(
    xMaxIndex,
    Math.ceil(xAtThisLevel / tileSizeAtThisLevel[0])
  );
  const yMinClamped = Math.max(yMinIndex, 0);
  const yMaxClamped = Math.min(
    yMaxIndex,
    Math.ceil(yAtThisLevel / tileSizeAtThisLevel[1])
  );
  let channelData = [];
  let displayData = [];
  // Now that you have the indices, you can fetch the tiles that are in the range [xMinClamped, xMaxClamped] and [yMinClamped, yMaxClamped]
  for (const [i, visible] of (channelsVisible || []).entries()) {
    if (visible) {
      let thisChannel = {};
      let channelSelection = selections[i];
      thisChannel.selection = channelSelection;
      thisChannel.color = colors[i];
      thisChannel.contrastLimits = contrastLimits[i];
      thisChannel.data = [];
      let indexArray = [];
      const radiusSquared = sizeAtPyramidLevel * sizeAtPyramidLevel;
      for (let y = yMinClamped; y <= yMaxClamped; y++) {
        for (let x = xMinClamped; x <= xMaxClamped; x++) {
          const tileData = await loaderAtThisLevel.getTile({
            x,
            y,
            selection: channelSelection,
          });
          for (let j = 0; j < tileData.data.length; j++) {
            const xIndex = x * tileSizeAtThisLevel[0] + (j % tileData.width);
            const yIndex =
              y * tileSizeAtThisLevel[1] + Math.floor(j / tileData.width);
            if (xIndex > xMin && xIndex <= xMax) {
              if (yIndex >= yMin && yIndex <= yMax) {
                const dx = xIndex - x_center;
                const dy = yIndex - y_center;
                const distanceSquared = dx * dx + dy * dy;
                if (distanceSquared <= radiusSquared) {
                  indexArray.push([xIndex, yIndex, tileData.data[j]]);
                  thisChannel.data.push(tileData.data[j]);
                }
              }
            }
          }
        }
      }
      channelData.push(thisChannel);
    }
  }
  setMovingLens(false);
  return channelData;
}

export async function optimizeInLens(channelData, contrastLimits) {
  let totalLength = 0;
  channelData.forEach((d) => {
    totalLength += d.data.length;
  });

  let allData = new Uint16Array(totalLength);
  let index = 0;
  let _colors = [];
  let _contrastLimits = [];
  let modifiedData = channelData.map((d) => {
    allData.set(d.data, index);
    index += d.data.length;
    _colors = [..._colors, ...d.color];
    _contrastLimits = [..._contrastLimits, ...d.contrastLimits];
    delete d.data;
    return d;
  });
  return psudoAnalysis.optimize_in_lens(allData, _colors, _contrastLimits);
}

export function savePastPalette(
  colors,
  channelsVisible,
  contrastLimits,
  selections,
  viewState
) {
  let pastPalettes = JSON.parse(localStorage.getItem("pastPalettes") || "[]");
  // Create unique id
  let id = Math.random().toString(36).substring(2, 15);
  let pastPalette = {
    id: id,
    colors: colors,
    channelsVisible: channelsVisible,
    contrastLimits: contrastLimits,
    selections: selections,
    viewState: viewState,
  };
  pastPalettes.push(pastPalette);
  localStorage.setItem("pastPalettes", JSON.stringify(pastPalettes));
  return pastPalettes;
}

export function getPastPalettes() {
  let pastPalettes = JSON.parse(localStorage.getItem("pastPalettes") || "[]");
  return pastPalettes;
}

export function deletePastPalette(id) {
  let pastPalettes = JSON.parse(localStorage.getItem("pastPalettes") || "[]");
  pastPalettes = pastPalettes.filter((d) => d.id != id);
  localStorage.setItem("pastPalettes", JSON.stringify(pastPalettes));
  return pastPalettes;
}

export function clearPastPalettes() {
  localStorage.setItem("pastPalettes", JSON.stringify([]));
  return [];
}
