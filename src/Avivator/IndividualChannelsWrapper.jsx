import React, { Fragment, useContext, useEffect, useState } from "react";
import { AppContext } from "../context/GlobalContext.jsx";
import "react-perfect-scrollbar/dist/css/styles.css";

import { Grid } from "@mui/material";
import _ from "lodash";
import { useChannelsStore, useLoader, useViewerStore } from "./state.js";
import shallow from "zustand/shallow";
import MiniAvivator from "./MiniAvivator.jsx";
import ChannelColorDisplay from "../components/ChannelColorDisplay.jsx";
import { createTheme, ThemeProvider } from "@mui/material/styles";
import { makeBoundingBox } from "@vivjs/layers";
import { GLOBAL_SLIDER_DIMENSION_FIELDS } from "./constants.js";
import GlobalSelectionSlider from "./components/Controller/components/GlobalSelectionSlider.jsx";

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




  return (
    <>
      <ThemeProvider theme={darkTheme}>
        {/* Add a rounded white border to this */}
        <div
          style={{
            maxHeight: "85vh",
            overflowY: "auto",
            overflowX: "hidden",
            marginTop: "8vh",
            marginBottom: "7vh",
            width: "100%",
            colorScheme: "dark",
          }}
        >
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
                sx={{ paddingRight: "20px", zIndex: 1000, minHeight: "200px" }}
              >
                <Grid item xs={6} sx={{ paddingRight: "30px" }}>
                  {channelsVisible?.[i] && (
                    <MiniAvivator allowNavigation={false} showChannel={i} />
                  )}
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
