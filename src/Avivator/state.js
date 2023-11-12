import create from "zustand";
import { getNameFromUrl } from "./viewerUtils.js";
const captialize = (string) => string.charAt(0).toUpperCase() + string.slice(1);

const generateToggles = (defaults, set) => {
  const toggles = {};
  Object.entries(defaults).forEach(([k, v]) => {
    if (typeof v === "boolean") {
      toggles[`toggle${captialize(k)}`] = () =>
        set((state) => ({
          ...state,
          [k]: !state[k],
        }));
    }
  });
  return toggles;
};

const DEFAUlT_CHANNEL_STATE = {
  channelsVisible: [],
  contrastLimits: [],
  colors: [],
  prevColors: [],
  domains: [],
  selections: [],
  ids: [],
  loader: [{ labels: [], shape: [] }],
  image: 0,
};

const DEFAUlT_CHANNEL_VALUES = {
  channelsVisible: true,
  contrastLimits: [0, 65535],
  colors: [255, 255, 255],
  prevColors: [255, 255, 255],
  domains: [0, 65535],
  selections: { z: 0, c: 0, t: 0 },
  ids: "",
};

export const useChannelsStore = create((set) => ({
  ...DEFAUlT_CHANNEL_STATE,
  ...generateToggles(DEFAUlT_CHANNEL_VALUES, set),
  toggleIsOn: (index) =>
    set((state) => {
      const channelsVisible = [...state.channelsVisible];
      channelsVisible[index] = !channelsVisible[index];
      return { ...state, channelsVisible };
    }),
  setPropertiesForChannel: (channel, newProperties) =>
    set((state) => {
      const entries = Object.entries(newProperties);
      const newState = {};
      entries.forEach(([property, value]) => {
        newState[property] = [...state[property]];
        newState[property][channel] = value;
      });
      return { ...state, ...newState };
    }),
  removeChannel: (channel) =>
    set((state) => {
      const newState = {};
      const channelKeys = Object.keys(DEFAUlT_CHANNEL_VALUES);
      Object.keys(state).forEach((key) => {
        if (channelKeys.includes(key)) {
          newState[key] = state[key].filter((_, j) => j !== channel);
        }
      });
      return { ...state, ...newState };
    }),
  addChannel: (newProperties) =>
    set((state) => {
      const entries = Object.entries(newProperties);
      const newState = { ...state };
      entries.forEach(([property, value]) => {
        newState[property] = [...state[property], value];
      });
      Object.entries(DEFAUlT_CHANNEL_VALUES).forEach(([k, v]) => {
        if (newState[k].length < newState[entries[0][0]].length) {
          newState[k] = [...state[k], v];
        }
      });
      return newState;
    }),
}));

const DEFAULT_IMAGE_STATE = {
  lensSelection: [0, 0, 0, 0, 0, 0],
  colormap: "sRGB",
  fragmentShader: null,
  resolution: 0,
  lensEnabled: true,
  zoomLock: false,
  panLock: false,
  isOverviewOn: false,
  useFixedAxis: true,
};

// const filePath = "/src/assets/exemplar-001.ome.tif";
// const response = await fetch(filePath);
// const data = await response.blob();
// const fetchedFile = new File([data], "exemplar-001.ome.tif", {
//   type: "image/tiff",
// });

export const useImageSettingsStore = create((set) => ({
  ...DEFAULT_IMAGE_STATE,
  ...generateToggles(DEFAULT_IMAGE_STATE, set),
}));

const DEFAULT_VIEWER_STATE = {
  isChannelLoading: [],
  isViewerLoading: true,
  pixelValues: [],
  coordinate: {},
  isOffsetsSnackbarOn: false,
  loaderErrorSnackbar: {
    on: false,
    message: null,
  },
  isNoImageUrlSnackbarOn: false,
  isVolumeRenderingWarningOn: false,
  useColormap: false,
  globalSelection: { z: 0, t: 0 },
  channelOptions: [],
  metadata: null,
  viewState: null,
  zoom: null,
  // source: {
  //   urlOrFile:fetchedFile,
  //   description: 'exemplar-001.ome.tif',
  // },
  //   source: {
  //     urlOrFile:
  //       "https://viv-demo.storage.googleapis.com/Vanderbilt-Spraggins-Kidney-MxIF.ome.tif",
  //     description: getNameFromUrl(
  //       "https://viv-demo.storage.googleapis.com/Vanderbilt-Spraggins-Kidney-MxIF.ome.tif"
  //     ),
  //   },
  //   // pyramidResolution: 0,
  //   pyramidResolution: 0,
  // };
  source: {},
};  

export const useViewerStore = create((set) => ({
  ...DEFAULT_VIEWER_STATE,
  ...generateToggles(DEFAULT_VIEWER_STATE, set),
  setIsChannelLoading: (index, val) =>
    set((state) => {
      const newIsChannelLoading = [...state.isChannelLoading];
      newIsChannelLoading[index] = val;
      return { ...state, isChannelLoading: newIsChannelLoading };
    }),
  addIsChannelLoading: (val) =>
    set((state) => {
      const newIsChannelLoading = [...state.isChannelLoading, val];
      return { ...state, isChannelLoading: newIsChannelLoading };
    }),
  removeIsChannelLoading: (index) =>
    set((state) => {
      const newIsChannelLoading = [...state.isChannelLoading];
      newIsChannelLoading.splice(index, 1);
      return { ...state, isChannelLoading: newIsChannelLoading };
    }),
}));

export const useLoader = () => {
  const [fullLoader, image] = useChannelsStore((store) => [
    store.loader,
    store.image,
  ]);
  return Array.isArray(fullLoader[0]) ? fullLoader[image] : fullLoader;
};

export const useMetadata = () => {
  const image = useChannelsStore((store) => store.image);
  const metadata = useViewerStore((store) => store.metadata);
  return Array.isArray(metadata) ? metadata[image] : metadata;
};
