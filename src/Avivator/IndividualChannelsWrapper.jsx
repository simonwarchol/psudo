import React, { Fragment, useContext, useEffect, useState } from "react";
import { AppContext } from "../context/GlobalContext.jsx";
import PerfectScrollbar from "react-perfect-scrollbar";
import "react-perfect-scrollbar/dist/css/styles.css";

import {
  Button,
  FormControlLabel,
  FormLabel,
  Grid,
  InputLabel,
  Menu,
  MenuItem,
  Paper,
  Radio,
  RadioGroup,
  Select,
} from "@mui/material";
import _ from "lodash";
import {
  useChannelsStore,
  useImageSettingsStore,
  useLoader,
  useViewerStore,
} from "./state.js";
import shallow from "zustand/shallow";
import MiniAvivator from "./MiniAvivator.jsx";
import ChannelColorDisplay from "../components/ChannelColorDisplay.jsx";
import { createTheme, ThemeProvider } from "@mui/material/styles";
import { makeBoundingBox } from "@vivjs/layers";
import FormControl from "@mui/material/FormControl";
import { GLOBAL_SLIDER_DIMENSION_FIELDS } from "./constants.js";
import GlobalSelectionSlider from "./components/Controller/components/GlobalSelectionSlider.jsx";
import AddChannel from "./components/Controller/components/AddChannel.jsx";
import ChannelList from "../components/ChannelList.jsx";
import LayersIcon from "@mui/icons-material/Layers";
import * as psudoAnalysis from "psudo-analysis";

// import lodash
function IndividualChannelsWrapper() {
  const context = useContext(AppContext);
  const darkTheme = createTheme({
    palette: {
      mode: "dark",
    },
  });

  const colorOptions = [
    { value: "Oklab", label: "OKLAB" },
    { value: "sRGB", label: "sRGB" },
  ];
  const [
    globalSelection,
    channelOptions,
    source,
    viewState,
    pyramidResolution,
  ] = useViewerStore(
    (store) => [
      store.globalSelection,
      store.channelOptions,
      store.source,
      store.viewState,
      store.pyramidResolution,
    ],
    shallow
  );

  const [colorSpace, setColorSpace] = useState(colorOptions[0].value);
  // useEffect(() => {
  //   if (!colorSpace) return;
  //   console.log("Changing", colorSpace);
  //   useImageSettingsStore.setState({ colormap: colorSpace });
  //   //     Color Space Update
  // }, [colorSpace]);
  const handleChange = (event) => {
    setColorSpace(event.target.value);
  };

  const loader = useLoader();
  const { shape, labels } = loader[0];
  const globalControlLabels = labels.filter((label) =>
    GLOBAL_SLIDER_DIMENSION_FIELDS.includes(label)
  );
  const globalControllers = globalControlLabels.map((label) => {
    const size = shape[labels.indexOf(label)];
    // Only return a slider if there is a "stack."
    return size > 1 ? (
      <GlobalSelectionSlider key={label} size={size} label={label} />
    ) : null;
  });

  const [anchorEl, setAnchorEl] = useState(null);
  const openEl = Boolean(anchorEl);
  const handleElClick = (event) => {
    setAnchorEl(event.currentTarget);
  };
  const handleCloseEl = () => {
    setAnchorEl(null);
  };

  let [channelsVisible, colors, contrastLimits, selections] = useChannelsStore(
    (store) => [
      store.channelsVisible,
      store.colors,
      store.contrastLimits,
      store.selections,
    ],
    shallow
  );
  const savePastPalette = async () => {
    const visibleIndices = selections
      .map((s) => s.c)
      .filter((d, i) => channelsVisible[i]);
    const fetchUrl = new URL(
      `${import.meta.env.VITE_BACKEND_URL}/save_past_palette`
    );
    const params = {
      pathName: context?.randomState?.dataPath,
      channelsVisible: JSON.stringify(visibleIndices),
      colorSpace: colorSpace,
      colors,
      contrastLimits: JSON.stringify(contrastLimits),
      optimizationScope:
        context.optimizationScope === "global"
          ? JSON.stringify({})
          : JSON.stringify(makeBoundingBox(viewState)),
      z: selections[0].z,
    };
    fetchUrl.search = new URLSearchParams(params).toString();
    const response = await fetch(fetchUrl);
    const newColors = response.ok ? await response.json() : {};
    console.log("New Colors", newColors);
    context.setPastPalettes([newColors, ...context.pastPalettes]);
  };

  const getNewPalette = async () => {
    context?.setIsLoading(true);
    await savePastPalette();
    const visibleIndices = selections
      .map((s) => s.c)
      .filter((d, i) => channelsVisible[i]);
    const fetchUrl = new URL(
      `${import.meta.env.VITE_BACKEND_URL}/get_auto_palette`
    );
    const params = {
      dataPath: source?.urlOrFile,
      colors: colors,
      lockedChannelColors: context.lockedChannelColors,
      channelColorNames: JSON.stringify(context.channelColorNames),
      colorexcluded: context.colorExcluded,
      colorSpace: colorSpace,
      contrastLimits: JSON.stringify(contrastLimits),
      channelsVisible: JSON.stringify(visibleIndices),
      globalSelection: JSON.stringify(globalSelection),
      optimizationScope:
        context.optimizationScope === "global"
          ? JSON.stringify({})
          : JSON.stringify(makeBoundingBox(viewState)),
      z: selections[0].z,
    };
    fetchUrl.search = new URLSearchParams(params).toString();
    const response = await fetch(fetchUrl);
    const newColors = response.ok ? await response.json() : [];
    let ctr = 0;
    const _tmpColors = _.cloneDeep(colors).map((color, i) => {
      if (channelsVisible[i]) {
        return newColors["newPalette"][ctr++];
      } else {
        return color;
      }
    });
    context?.setIsLoading(false);

    context?.setShowOptimizedColor(true);
    if (_tmpColors)
      useChannelsStore.setState({ colors: _tmpColors, prevColors: colors });
  };

  return (
    <>
      <ThemeProvider theme={darkTheme}>
        {/* Add a rounded white border to this */}
          <div style={{ maxHeight: "85vh", overflowY: "auto", overflowX: 'hidden', marginTop:'8vh', marginBottom: '7vh', width:'100%', colorScheme:'dark' }}>
            <Grid
              container
              direction="column"
              item
              justifyContent="center"
              alignItems="center"
              maxHeight="300vh"
            >
              {selections.map((d, i) => (
                <Grid
                  container
                  item
                  key={`mini__avivator_${i}_${JSON.stringify(i)}`}
                  sx={{paddingRight:'20px', zIndex:1000, minHeight:'200px'}}
                >
                  <Grid item xs={6} sx={{paddingRight:'30px'}} >
                    <MiniAvivator
                      allowNavigation={false}
                      showChannel={i}
                    />
                  </Grid>
                  <Grid item xs={6}>
                    <ChannelColorDisplay channelIndex={i} />
                  </Grid>
                </Grid>
              ))}
            </Grid>
          </div>
      </ThemeProvider>
    </>
  );
}

export default IndividualChannelsWrapper;
