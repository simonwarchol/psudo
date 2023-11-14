// @ts-nocheck
import shallow from "zustand/shallow";
import { debounce } from "lodash";

import { LensExtension } from "@hms-dbmi/viv";
import { VivView } from "@hms-dbmi/viv";
import { CompositeLayer, COORDINATE_SYSTEM } from "@deck.gl/core";
import { bin } from "d3-array";
import {
  getChannelGraphData,
  getLensIntensityValues,
  getGraphData,
  optimizeInLens,
  calculateLensPaletteLoss,
} from "./viewerUtils.js";

import {
  ScatterplotLayer,
  PolygonLayer,
  SolidPolygonLayer,
  PathLayer,
  IconLayer,
} from "@deck.gl/layers";
import _ from "lodash";
import * as psudoAnalysis from "psudo-analysis";
import { MultiscaleImageLayer, ImageLayer } from "@vivjs/layers";
import srgbFs from "../components/shaders/fragment-shaders/rgb-color-mixing-fs.glsl";
import oklabFs from "../components/shaders/fragment-shaders/oklab-color-mixing-fs.glsl";
import {
  useImageSettingsStore,
  useViewerStore,
  useChannelsStore,
} from "./state.js";

const defaultProps = {
  lensEnabled: { type: "boolean", value: true, compare: true },
  lensSelection: { type: "array", value: [0, 0, 0, 0, 0, 0], compare: true },
  lensRadius: { type: "number", value: 100, compare: true },
  lensOpacity: { type: "number", value: 1.0, compare: true },
  overlapView: { type: "boolean", value: false, compare: true },
  lensBorderColor: { type: "array", value: [255, 255, 255], compare: true },
  lensBorderRadius: { type: "number", value: 0.02, compare: true },
  colors: { type: "array", value: null, compare: true },
};

function getImageLayer(id, props) {
  const { loader } = props;
  // Grab name of PixelSource if a class instance (works for Tiff & Zarr).
  const sourceName = loader[0]?.constructor?.name;

  // Create at least one layer even without selections so that the tests pass.
  const Layer = loader.length > 1 ? MultiscaleImageLayer : ImageLayer;
  const layerLoader = loader.length > 1 ? loader : loader[0];

  return new Layer({
    ...props,
    id: `${sourceName}${getVivId(id)}`,
    viewportId: id,
    loader: layerLoader,
  });
}

function getVivId(id) {
  return `-#${id}#`;
}

const updateLensGraphValues = async (
  coordinate,
  viewState,
  loader,
  pyramidResolution,
  lensRadius,
  channelsVisible,
  selections,
  setMovingLens,
  colors,
  lensSelection,
  setGraphData,
  setPaletteLoss
) => {
  const channelData = await getLensIntensityValues(
    coordinate,
    viewState,
    loader,
    pyramidResolution,
    lensRadius,
    channelsVisible,
    selections,
    setMovingLens,
    useChannelsStore.getState()?.contrastLimits,
    useChannelsStore.getState()?.colors
  );

  let graphData = getGraphData(
    channelData,
    colors,
    lensSelection,
    channelsVisible,
    selections
  );

  setGraphData(graphData);
  console.log("getLensIntensityValues", channelData);
  let paletteLoss = await calculateLensPaletteLoss(channelData);
  setPaletteLoss(paletteLoss);
};

const debouncedUpdateLensGraphValues = debounce(updateLensGraphValues, 1000);

const MinervaVivLensing = class extends LensExtension {
  getShaders() {
    const self = this;
    const getFs = (colormap) => {
      switch (colormap) {
        case "sRGB":
          return srgbFs;
        case "Oklab":
          return oklabFs;
        default:
          return oklabFs;
      }
    };

    // For some reason this is called twice for the base class and parent class,
    // so we need to check if the parent class has the colormap property
    const colormap = self?.opts?.colormap || self?.props?.colormap;
    // console.log('getShadersSelf', self, colormap);
    const fragmentShader = getFs(colormap);
    useImageSettingsStore.setState({ fragmentShader, colormap });
    return {
      ...super.getShaders(),
      modules: [
        {
          name: "minerva-lens-module",
          fs: fragmentShader,
          inject: {
            "fs:DECKGL_MUTATE_COLOR": `
                       vec3 rgb = rgba.rgb;
                       mutate_color(rgb, intensity0, intensity1, intensity2, intensity3, intensity4, intensity5, vTexCoord);
                       rgba = vec4(rgb, 1.);
                      `,
          },
        },
      ],
    };
  }
  initializeState() {
    // super.initializeState();
    if (this.context.deck) {
      this.context.deck.eventManager.on({
        pan: () => null,
        pointermove: () => null,
        pointerleave: () => null,
        wheel: () => null,
      });
    }
  }

  draw() {
    const layer = this.getCurrentLayer();
    const { viewportId } = layer.props;
    const { lensRadius = defaultProps.lensRadius.value } =
      this.parent.context.userData;
    const { lensOpacity = defaultProps.lensOpacity.value } =
      this.parent.context.userData;
    const { overlapView = defaultProps.overlapView.value } =
      this.parent.context.userData;
    // If there is no viewportId, don't try to do anything.
    if (!viewportId) {
      layer.setState({ unprojectLensBounds: [0, 0, 0, 0] });
      return;
    }
    const mousePosition = {
      x: this.parent.context.userData.mousePosition[0],
      y: this.parent.context.userData.mousePosition[1],
    };
    const layerView = layer.context.deck.viewManager.views.filter(
      (view) => view.id === viewportId
    )[0];
    const viewState = layer.context.deck.viewManager.viewState[viewportId];
    const viewport = layerView.makeViewport({
      ...viewState,
      viewState,
    });
    // If the mouse is in the viewport and the mousePosition exists, set
    // the state with the bounding box of the circle that will render as a lens.
    if (mousePosition && viewport.containsPixel(mousePosition)) {
      const offsetMousePosition = {
        x: mousePosition.x - viewport.x,
        y: mousePosition.y - viewport.y,
      };
      const mousePositionBounds = [
        // left
        [offsetMousePosition.x - lensRadius, offsetMousePosition.y],
        // bottom
        [offsetMousePosition.x, offsetMousePosition.y + lensRadius],
        // right
        [offsetMousePosition.x + lensRadius, offsetMousePosition.y],
        // top
        [offsetMousePosition.x, offsetMousePosition.y - lensRadius],
      ];
      // Unproject from screen to world coordinates.
      const unprojectLensBounds = mousePositionBounds.map(
        (bounds, i) => viewport.unproject(bounds)[i % 2]
      );
      layer.setState({ unprojectLensBounds });
    } else {
      layer.setState({ unprojectLensBounds: [0, 0, 0, 0] });
    }
    this.state.model?.setUniforms({
      lensOpacity: lensOpacity,
      overlapView: overlapView,
    });
    super.draw();
  }
  updateState({ props, oldProps, changeFlags, ...rest }) {
    super.updateState({ props, oldProps, changeFlags, ...rest });
    if (props.colormap !== oldProps.colormap) {
      const { gl } = this.context;
      if (this.state.model) {
        this.state.model.delete();
        this.setState({ model: this._getModel(gl) });
      }
    }
  }
};

const LensLayer = class extends CompositeLayer {
  constructor(props) {
    super(props);
  }

  renderLayers() {
    const { id, viewState, loader } = this.props;
    const showLens = useImageSettingsStore.getState()?.lensEnabled;
    const lensSelection = useImageSettingsStore.getState()?.lensSelection;
    const mousePosition = this.context.userData.mousePosition || [
      Math.round((this.context.deck.width || 0) / 2),
      Math.round((this.context.deck.height || 0) / 2),
    ];
    this.lensPosition =
      this.context.deck.pickObject({
        x: mousePosition[0],
        y: mousePosition[1],
        radius: 1,
      })?.coordinate || viewState.target;
    if (_.isEmpty(this.context.userData.coordinate)) {
      this.context.userData.setCoordinate(this.lensPosition);
    }
    if (!showLens) return [];

    this.lensPosition =
      this.context.deck.pickObject({
        x: mousePosition[0],
        y: mousePosition[1],
        radius: 1,
      })?.coordinate || viewState.target;
    //     const colors = useChannelsStore.getState()?.colors;

    if (this.context.userData?.mainViewStateChanged) {
      console.log("Setpal", this.context.userData?.paletteLoss);
      debouncedUpdateLensGraphValues(
        this.lensPosition,
        viewState,
        loader,
        this.context.userData?.pyramidResolution,
        this.context.userData?.lensRadius,
        this.context.userData?.channelsVisible,
        this.context.userData?.selections,
        this.context.userData?.setMovingLens,
        useChannelsStore.getState()?.colors,
        lensSelection,
        this.context.userData?.setGraphData,
        this.context.userData?.setPaletteLoss
      );
      this.context.userData.setMainViewStateChanged(false);
    }

    const lensCircle = new ScatterplotLayer({
      id: `lens-circle-${id}`,
      coordinateSystem: COORDINATE_SYSTEM.CARTESIAN,
      data: [this.lensPosition],
      pickable: true,
      animate: true,
      // opacity: 0.5,
      stroked: true,
      alphaCutoff: 0,
      filled: true,
      updateTriggers: {
        getPosition: Date.now() % 2000,
      },

      getFillColor: (d) => [0, 0, 0, 0],
      lineWidthMinPixels: 1,
      getPosition: (d) => {
        return d;
      },
      getRadius: (d) => {
        let multiplier = 1 / Math.pow(2, viewState.zoom);
        const size = this.context.userData.lensRadius * multiplier;
        return size;
      },
      getLineColor: (d) => [255, 255, 255],
      getLineWidth: (d) => {
        const multiplier = 1 / Math.pow(2, viewState.zoom);
        return 3 * multiplier;
      },
    });

    const resizeCircle = new ScatterplotLayer({
      id: `resize-circle-${id}`,
      coordinateSystem: COORDINATE_SYSTEM.CARTESIAN,
      data: [this.lensPosition],
      pickable: true,
      animate: true,
      // opacity: 0.5,
      stroked: true,
      alphaCutoff: 0,
      filled: true,
      updateTriggers: {
        getPosition: Date.now() % 2000,
      },

      getFillColor: (d) => [0, 0, 0, 0],
      lineWidthMinPixels: 1,
      getPosition: (d) => {
        let multiplier = 1 / Math.pow(2, viewState.zoom);
        const resizeRadius = 20 * multiplier;
        const lensRadius = this.context.userData.lensRadius * multiplier;
        const distanceFromCenter = lensRadius + resizeRadius; // Adjusts distance between lens and circle
        const dx = Math.cos(Math.PI / 4) * distanceFromCenter;
        const dy = Math.sin(Math.PI / 4) * distanceFromCenter;
        return [d[0] + dx, d[1] + dy];
      },
      getRadius: (d) => {
        let multiplier = 1 / Math.pow(2, viewState.zoom);
        const resizeRadius = 20;

        const size = resizeRadius * multiplier;
        return size;
      },
      getLineColor: (d) => [255, 255, 255],
      getLineWidth: (d) => {
        const multiplier = 1 / Math.pow(2, viewState.zoom);
        return 3 * multiplier;
      },
    });

    // SVG points
    const svgPoints = [
      [190.367, 316.44],
      [190.367, 42.226],
      [236.352, 88.225],
      [251.958, 72.619],
      [179.333, 0],
      [106.714, 72.613],
      [122.291, 88.231],
      [168.302, 42.226],
      [168.302, 316.44],
      [122.314, 270.443],
      [106.708, 286.044],
      [179.333, 358.666],
      [251.958, 286.056],
      [236.363, 270.432],
    ];

    const avgPoint = svgPoints.reduce(
      (acc, point) => [
        acc[0] + point[0] / svgPoints.length,
        acc[1] + point[1] / svgPoints.length,
      ],
      [0, 0]
    );

    const normalizedSvgPoints = svgPoints.map((point) => [
      point[0] - avgPoint[0],
      point[1] - avgPoint[1],
    ]);

    const arrowLayer = new SolidPolygonLayer({
      id: `arrow-layer-${id}`,
      coordinateSystem: COORDINATE_SYSTEM.CARTESIAN,
      data: [this.lensPosition],
      getPolygon: (d) => {
        let multiplier = 1 / Math.pow(2, viewState.zoom);
        const resizeRadius = 20 * multiplier;
        const lensRadius = this.context.userData.lensRadius * multiplier;
        const distanceFromCenter = lensRadius + resizeRadius;
        const dx = Math.cos(Math.PI / 4) * distanceFromCenter;
        const dy = Math.sin(Math.PI / 4) * distanceFromCenter;
        const center = [d[0] + dx, d[1] + dy];

        const scale = 0.1 * multiplier;

        const rotatePoint = (point, angle) => {
          const cos = Math.cos(angle);
          const sin = Math.sin(angle);
          const [x, y] = point;
          const rotatedX =
            cos * (x - center[0]) - sin * (y - center[1]) + center[0];
          const rotatedY =
            sin * (x - center[0]) + cos * (y - center[1]) + center[1];
          return [rotatedX, rotatedY];
        };

        // Rotate each SVG point by 45 degrees about its center, then scale and position them
        const transformedPoints = normalizedSvgPoints.map((point) => {
          const scaledPoint = [
            center[0] + point[0] * scale,
            center[1] + point[1] * scale,
          ];
          return rotatePoint(scaledPoint, -Math.PI / 4);
        });

        return transformedPoints;
      },
      getFillColor: [53, 121, 246],
      extruded: false,
      pickable: false,
    });

    const opacityLayer = new PolygonLayer({
      id: `opacity-layer-${id}`,
      coordinateSystem: COORDINATE_SYSTEM.CARTESIAN,
      data: [this.lensPosition],
      getPolygon: (d) => {
        const opacity = this.context.userData.lensOpacity;
        const angle = (3 * Math.PI) / 2 - ((0.5 - opacity) * Math.PI) / 2;
        let multiplier = 1 / Math.pow(2, viewState.zoom);

        const lensRadius = this.context.userData.lensRadius * multiplier;

        const centerOfSemiCircle = [
          d[0] + Math.cos(angle) * lensRadius,
          d[1] + Math.sin(angle) * lensRadius,
        ];
        const size = 20 * multiplier;

        // Generate semicircle points
        const semiCirclePoints = [];
        for (
          let theta = angle + Math.PI / 2;
          theta <= (3 * Math.PI) / 2 + angle;
          theta += Math.PI / 36
        ) {
          // Change the denominator for more or fewer points
          semiCirclePoints.push([
            centerOfSemiCircle[0] - size * Math.cos(theta),
            centerOfSemiCircle[1] - size * Math.sin(theta),
          ]);
        }

        // Add center of the semicircle to close the shape
        // semiCirclePoints.push(centerOfSemiCircle);

        return semiCirclePoints;
      },
      getFillColor: [0, 0, 0, 0],
      getLineWidth: (d) => {
        const multiplier = 1 / Math.pow(2, viewState.zoom);
        return 3 * multiplier;
      },
      extruded: false,
      pickable: true,
      alphaCutoff: 0,
      stroked: true,
      getLineColor: [255, 255, 255],
    });

    const contrastSemiCircle = new PolygonLayer({
      id: `contrast-semi-layer-${id}`,
      coordinateSystem: COORDINATE_SYSTEM.CARTESIAN,
      data: [this.lensPosition],
      getPolygon: (d) => {
        const angle = 0;
        let multiplier = 1 / Math.pow(2, viewState.zoom);
        const lensRadius = this.context.userData.lensRadius * multiplier;
        const size = 20 * multiplier;
        const innerRadius = size * 0.6;

        const centerOfSemiCircle = [
          d[0] + Math.cos((3 * Math.PI) / 4) * (lensRadius + size),
          d[1] + Math.sin((3 * Math.PI) / 4) * (lensRadius + size),
        ];

        // Generate semicircle points
        const semiCirclePoints = [];
        for (
          let theta = angle + Math.PI / 2;
          theta <= (3 * Math.PI) / 2 + angle;
          theta += Math.PI / 36
        ) {
          // Change the denominator for more or fewer points
          semiCirclePoints.push([
            centerOfSemiCircle[0] - innerRadius * Math.cos(theta),
            centerOfSemiCircle[1] - innerRadius * Math.sin(theta),
          ]);
        }

        // Add center of the semicircle to close the shape
        // semiCirclePoints.push(centerOfSemiCircle);

        return semiCirclePoints;
      },
      getFillColor: [255, 255, 255, 255],
      getLineWidth: (d) => {
        const multiplier = 1 / Math.pow(2, viewState.zoom);
        return 3 * multiplier;
      },
      extruded: false,
      pickable: false,
      alphaCutoff: 0,
      stroked: true,
      getLineColor: [255, 255, 255],
    });

    const contrastCircle = new ScatterplotLayer({
      id: `contrast-circle-${id}`,
      coordinateSystem: COORDINATE_SYSTEM.CARTESIAN,
      data: [this.lensPosition],
      pickable: true,
      animate: true,
      // opacity: 0.5,
      stroked: true,
      alphaCutoff: 0,
      filled: true,
      updateTriggers: {
        getPosition: Date.now() % 2000,
      },

      getFillColor: (d) => [0, 0, 0, 0],
      lineWidthMinPixels: 1,
      getPosition: (d) => {
        let multiplier = 1 / Math.pow(2, viewState.zoom);
        const resizeRadius = 20 * multiplier;
        const lensRadius = this.context.userData.lensRadius * multiplier;
        const distanceFromCenter = lensRadius + resizeRadius; // Adjusts distance between lens and circle
        const dx = Math.cos((3 * Math.PI) / 4) * distanceFromCenter;
        const dy = Math.sin((3 * Math.PI) / 4) * distanceFromCenter;
        return [d[0] + dx, d[1] + dy];
      },
      getRadius: (d) => {
        let multiplier = 1 / Math.pow(2, viewState.zoom);
        const resizeRadius = 20;

        const size = resizeRadius * multiplier;
        return size;
      },
      getLineColor: (d) => [255, 255, 255],
      getLineWidth: (d) => {
        const multiplier = 1 / Math.pow(2, viewState.zoom);
        return 3 * multiplier;
      },
    });

    const graphCircle = new ScatterplotLayer({
      id: `graph-circle-${id}`,
      coordinateSystem: COORDINATE_SYSTEM.CARTESIAN,
      data: [this.lensPosition],
      pickable: true,
      animate: true,
      // opacity: 0.5,
      stroked: true,
      alphaCutoff: 0,
      filled: true,
      updateTriggers: {
        getPosition: Date.now() % 2000,
      },

      getFillColor: (d) => [0, 0, 0, 0],
      lineWidthMinPixels: 1,
      getPosition: (d) => {
        let multiplier = 1 / Math.pow(2, viewState.zoom);
        const resizeRadius = 20 * multiplier;
        const lensRadius = this.context.userData.lensRadius * multiplier;
        const distanceFromCenter = lensRadius + resizeRadius; // Adjusts distance between lens and circle
        const dx = Math.cos((2 * Math.PI) / 4) * distanceFromCenter;
        const dy = Math.sin((2 * Math.PI) / 4) * distanceFromCenter;
        return [d[0] + dx, d[1] + dy];
      },
      getRadius: (d) => {
        let multiplier = 1 / Math.pow(2, viewState.zoom);
        const resizeRadius = 20;

        const size = resizeRadius * multiplier;
        return size;
      },
      getLineColor: (d) => [255, 255, 255],
      getLineWidth: (d) => {
        const multiplier = 1 / Math.pow(2, viewState.zoom);
        return 3 * multiplier;
      },
    });

    const graphIconData = {
      graph1: [
        [25.5, 24.58],
        [37.32, 4.77],
        [50, 24.65],
        [50, 16],
        [37.25, -4],
        [24.38, 17.57],
        [12.05, 13.45],
        [0, 21.25],
        [0, 28.22],
        [12.4, 20.2],
        [25.5, 24.58],
      ],
      graph2: [
        [50, 28.27],
        [37.38, 23.3],
        [24.35, 30.95],
        [12.1, 29.65],
        [0, 32.85],
        [0, 39.43],
        [12.3, 36.18],
        [24.9, 37.52],
        [37.6, 30.08],
        [50, 34.95],
        [50, 28.27],
      ],
    };
    // Function to calculate average point
    const calculateAveragePoint = (points) => {
      const total = points.reduce(
        (acc, [x, y]) => [acc[0] + x, acc[1] + y],
        [0, 0]
      );
      return total.map((coord) => coord / points.length);
    };

    // Function to normalize points
    const normalizePoints = (points, averagePoint) => {
      return points.map(([x, y]) => [x - averagePoint[0], y - averagePoint[1]]);
    };

    // Combined points from both graphs
    const combinedPoints = [...graphIconData.graph1, ...graphIconData.graph2];
    const averagePoint = calculateAveragePoint(combinedPoints);

    // Normalized points for both graphs
    const normalizedPoints = {
      graph1: normalizePoints(graphIconData.graph1, averagePoint),
      graph2: normalizePoints(graphIconData.graph2, averagePoint),
    };

    // Function to create a graph layer
    const createGraphLayer = (id, graphKey, color) => {
      return new SolidPolygonLayer({
        id: `graph-icon-${graphKey}-layer-${id}`,
        coordinateSystem: COORDINATE_SYSTEM.CARTESIAN,
        data: [this.lensPosition],
        getPolygon: (d) => {
          const multiplier = 1 / Math.pow(2, viewState.zoom);
          const resizeRadius = 20 * multiplier;
          const lensRadius = this.context.userData.lensRadius * multiplier;
          const distanceFromCenter = lensRadius + resizeRadius;
          const angle = (2 * Math.PI) / 4;
          const dx = Math.cos(angle) * distanceFromCenter;
          const dy = Math.sin(angle) * distanceFromCenter;
          const center = [d[0] + dx, d[1] + dy];
          const scale = 0.5 * multiplier;

          return normalizedPoints[graphKey].map(([x, y]) => [
            center[0] + x * scale,
            center[1] + y * scale,
          ]);
        },
        getFillColor: color,
        extruded: false,
        pickable: false,
      });
    };

    // Graph icon layers
    const graphIconLayers = [
      createGraphLayer(id, "graph1", [53, 121, 246]),
      createGraphLayer(id, "graph2", [246, 121, 53]),
    ];

    // if (_.size(this.context.userData.graphData) > 0){
    const pathLayers = (this.context?.userData?.graphData || []).map(
      (graphD) => {
        return new PathLayer({
          id: "path-layer",
          data: [this.lensPosition],
          pickable: true,
          widthScale: 20,
          widthMinPixels: 2,
          getPath: (d) => {
            console.log("ddddd", graphD);
            let multiplier = 1 / Math.pow(2, viewState.zoom);
            const resizeRadius = 20 * multiplier;
            const lensRadius = this.context.userData.lensRadius;
            const center = [d[0], d[1]];
            const scale = multiplier * lensRadius * 0.6;

            const freqPoints = graphD?.data?.frequencies || [];

            const avgPoint = freqPoints.reduce(
              (acc, point) => [
                acc[0] + point[0] / freqPoints.length,
                acc[1] + point[1] / freqPoints.length,
              ],
              [0, 0]
            );

            const normalizedGraphPoints = freqPoints.map((point) => [
              (point[0] - avgPoint[0]) / 30,
              (point[1] - avgPoint[1]) / 0.5,
            ]);

            // Rotate each SVG point by 45 degrees about its center, then scale and position them
            const transformedPoints = normalizedGraphPoints.map((point) => {
              return [
                center[0] + point[0] * scale,
                center[1] + point[1] * -scale * 0.5,
              ];
            });

            return transformedPoints;
          },
          getColor: (d) => graphD.color,
          getWidth: (d) => {
            let multiplier = 1 / Math.pow(2, viewState.zoom);
            return 0.1 * multiplier;
          },
        });
      }
    );

    return [
      lensCircle,
      resizeCircle,
      arrowLayer,
      contrastCircle,
      graphCircle,
      ...graphIconLayers,
      contrastSemiCircle,
      // pathLayers,
      _.every(lensSelection, (num) => num === 0) ? null : opacityLayer,
    ];
  }
  onDrag(pickingInfo, event) {
    // console.log("Drag", pickingInfo?.sourceLayer?.id);
    const { viewState } = this.props;
    this.context.userData.setMovingLens(true);
    // this.context.userData.setGraphData([]);

    if (pickingInfo?.sourceLayer?.id === `resize-circle-${this.props.id}`) {
      const lensCenter = this.context.userData.mousePosition;
      // console.log("lensCenter", lensCenter, "event", event.offsetCenter);
      const xIntercept =
        (lensCenter[0] -
          lensCenter[1] +
          event.offsetCenter.x +
          event.offsetCenter.y) /
        2;
      const yIntercept = xIntercept + lensCenter[1] - lensCenter[0];
      const dx = xIntercept - lensCenter[0];
      const dy = yIntercept - lensCenter[1];
      const distance = Math.sqrt(dx * dx + dy * dy);
      const resizeRadius = 20;
      const newRadius =
        distance - resizeRadius > 35 ? distance - resizeRadius : 35;

      this.context.userData.setLensRadius(newRadius);
    } else if (
      pickingInfo?.sourceLayer?.id === `opacity-layer-${this.props.id}`
    ) {
      // console.log("Opacity");
      const lensCenter = this.context.userData.mousePosition;
      const angle = Math.atan2(
        lensCenter[1] - event.offsetCenter.y,
        lensCenter[0] - event.offsetCenter.x
      );
      let opacity;
      if (angle < Math.PI / 4 && angle > -Math.PI / 2) {
        opacity = 0;
      } else if (angle > (3 * Math.PI) / 4 || angle < -Math.PI / 2) {
        opacity = 1;
      } else {
        opacity = (angle - Math.PI / 4) / (Math.PI / 2);
      }

      this.context.userData.setLensOpacity(opacity);

      // Calcualte angle between event.offsetCenter\ and lensCenter
    } else if (
      pickingInfo?.sourceLayer?.id === `contrast-circle-${this.props.id}`
    ) {
    } else {
      // console.log("pickingInfo", pickingInfo.sourceLayer.id);
      this.context.userData.setMousePosition([
        event.offsetCenter.x,
        event.offsetCenter.y,
      ]);
    }
  }

  async onClick(pickingInfo, event) {
    this.context.userData.setMovingLens(false);
    const coordinate = this.context?.userData?.coordinate;

    // Determine which tiles are displayed
    const { viewState } = this.props;
    const { loader } = this.props;
    // this.context?.setIsLoading(true);

    if (pickingInfo?.sourceLayer?.id == `contrast-circle-${this.props.id}`) {
      const lensSelection = useImageSettingsStore.getState()?.lensSelection;

      const channelData = await getLensIntensityValues(
        coordinate,
        viewState,
        loader,
        this.context.userData?.pyramidResolution,
        this.context.userData?.lensRadius,
        this.context.userData?.channelsVisible,
        this.context.userData?.selections,
        this.context.userData?.setMovingLens,
        useChannelsStore.getState()?.contrastLimits,
        useChannelsStore.getState()?.colors
      );
      this.context?.userData?.setIsLoading(true);
      // If nothing is in the lens, apply to all channels
      if (_.every(lensSelection, (num) => num === 0)) {
        (this.context?.userData?.channelsVisible || []).forEach((d, i) => {
          if (d == true) {
            const selection = this.context?.userData?.selections[i];
            let thisChannelsData = channelData.filter((d) => {
              return _.isEqual(d.selection, selection);
            })[0];
            console.log("thisChannelsData", thisChannelsData);
            const conrastLimits = psudoAnalysis.channel_gmm(
              thisChannelsData.data
            );

            const intContrastLimits = [
              _.toInteger(conrastLimits[0]),
              _.toInteger(conrastLimits[1]),
            ];
            useChannelsStore.getState()?.setPropertiesForChannel(i, {
              contrastLimits: intContrastLimits,
            });
            // setPropertiesForChannel(i, { contrastLimits: intContrastLimits });
          }
        });
      } else {
        lensSelection.forEach((d, i) => {
          if (d == 1) {
            const selection = this.context?.userData?.selections[i];
            let thisChannelsData = channelData.filter((d) => {
              return _.isEqual(d.selection, selection);
            })[0];
            console.log("thisChannelsData", thisChannelsData);
            const conrastLimits = psudoAnalysis.channel_gmm(
              thisChannelsData.data
            );
            const intContrastLimits = [
              _.toInteger(conrastLimits[0]),
              _.toInteger(conrastLimits[1]),
            ];
            useChannelsStore.getState()?.setPropertiesForChannel(i, {
              contrastLimits: intContrastLimits,
            });
            // setPropertiesForChannel(i, { contrastLimits: intContrastLimits });
          }
        });
      }
      this.context?.userData?.setIsLoading(false);
    } else if (
      pickingInfo?.sourceLayer?.id == `graph-circle-${this.props.id}`
    ) {
      const lensSelection = useImageSettingsStore.getState()?.lensSelection;

      const channelData = await getLensIntensityValues(
        coordinate,
        viewState,
        loader,
        this.context.userData?.pyramidResolution,
        this.context.userData?.lensRadius,
        this.context.userData?.channelsVisible,
        this.context.userData?.selections,
        this.context.userData?.setMovingLens,
        useChannelsStore.getState()?.contrastLimits,
        useChannelsStore.getState()?.colors
      );
      console.log("channelData", channelData);
      let opt = await optimizeInLens(channelData);
      console.log("opt", opt);
    } else if (
      pickingInfo?.sourceLayer?.id == `resize-circle-${this.props.id}`
    ) {
      console.log("sourceLayer", pickingInfo?.sourceLayer?.id);
      console.time("optimize");
      const test = psudoAnalysis.optimize([
        127, 201, 127, 190, 174, 212, 253, 192, 134, 255, 255, 153, 56, 108,
        176, 240, 2, 127,
      ]);
      console.timeEnd("optimize");
      console.log("test", test);
    }
  }

  async onDragEnd(pickingInfo, event, d, e) {
    const { viewState } = this.props;
    const { loader } = this.props;
    const coordinate = pickingInfo?.coordinate;
    console.log("coordinate", coordinate);
    this.context.userData.setCoordinate(coordinate);
    this.context.userData.setMovingLens(false);
    const lensSelection = useImageSettingsStore.getState()?.lensSelection;
    const colors = useChannelsStore.getState()?.colors;
    await updateLensGraphValues(
      coordinate,
      viewState,
      loader,
      this.context.userData?.pyramidResolution,
      this.context.userData?.lensRadius,
      this.context.userData?.channelsVisible,
      this.context.userData?.selections,
      this.context.userData?.setMovingLens,
      colors,
      lensSelection,
      this.context.userData?.setGraphData,
      this.context.userData?.setPaletteLoss,
    );
  }
};
LensLayer.layerName = "LensLayer";
LensLayer.defaultProps = defaultProps;

class MinervaVivLensingDetailView extends VivView {
  constructor(props) {
    super(props);
    this.mousePosition = props?.mousePosition || [null, null];
    this.lensRadius = props?.lensRadius;
    this.lensOpacity = props?.lensOpacity;
    this.overlapView = props?.overlapView;
  }
  getLayers({ props, viewStates }) {
    const { loader } = props;
    const { id, height, width } = this;
    const layerViewState = viewStates[id];
    const layers = [getImageLayer(id, props)];

    // Inspect the first pixel source for physical sizes
    if (loader[0]?.meta?.physicalSizes?.x) {
      const { size, unit } = loader[0].meta.physicalSizes.x;
      layers.push(
        new LensLayer({
          id: getVivId(id),
          loader,
          unit,
          size,
          lensMousePosition: this.mousePosition,
          lensRadius: this.lensRadius,
          lensOpacity: this.lensOpacity,
          overlapView: this.overlapView,
          viewState: { ...layerViewState, height, width },
        })
      );
    }

    return layers;
  }
}

export { MinervaVivLensing, MinervaVivLensingDetailView };
