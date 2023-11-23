import React, { useContext, useEffect } from "react";
import { AppContext } from "../context/GlobalContext.jsx";
import { Grid, MenuItem } from "@mui/material";
import { createTheme, ThemeProvider } from "@mui/material/styles";
import {
  savePastPalette,
  clearPastPalettes,
  getPastPalettes,
  deletePastPalette,
} from "../Avivator/viewerUtils.js";
import PngCanvas from "./PngCanvas.jsx";
import {
  useChannelsStore,
  useImageSettingsStore,
  useViewerStore,
  useLoader,
} from "../Avivator/state.js";
import shallow from "zustand/shallow";
import { makeBoundingBox } from "@vivjs/layers";
import SaveIcon from "@mui/icons-material/Save";
import ClearIcon from "@mui/icons-material/Clear";
import RemoveIcon from "@mui/icons-material/Remove";

const PastPalettes = (props) => {
  const context = useContext(AppContext);
  const darkTheme = createTheme({ palette: { mode: "dark" } });
  const loader = useLoader();

  let [channelsVisible, colors, contrastLimits, selections] = useChannelsStore(
    (store) => [
      store.channelsVisible,
      store.colors,
      store.contrastLimits,
      store.selections,
    ],
    shallow
  );
  const colormap = useImageSettingsStore((store) => store.colormap);

  const [viewState, pyramidResolution] = useViewerStore(
    (store) => [store.viewState, store.pyramidResolution],
    shallow
  );

  useEffect(() => {
    console.log("pp, ", context?.pastPalettes);
  }, [context?.pastPalettes]);

  const selectOldPalette = (palette) => {

    useChannelsStore.setState({ colors: palette.colors, contrastLimits: palette.contrastLimits, selections: palette.selections, channelsVisible: palette.channelsVisible });
    useViewerStore.setState({ viewState: palette.viewState });
};

  //   const selectOldPalette = async (palette) => {
  //     console.log("clickclick", palette);
  //     if (
  //       context.pastPalettes?.[0] &&
  //       !context.pastPalettes?.[0]?.fromHistoryClick
  //     ) {
  //       await savePastPalette();
  //     }
  //     const newdata = [];
  //     for (let i = 0; i < palette.colors.length; i += 3) {
  //       // i+=3 can solve your problem
  //       const three = [
  //         palette.colors[i],
  //         palette.colors[i + 1],
  //         palette.colors[i + 2],
  //       ];
  //       newdata.push(three);
  //     }
  //     // transform colors into list of [rgb]

  //     // const oldPaletteColors = palette?.['channel_indices']?.map(d => {
  //     //     const channelColor = [palette['colors'][d * 3], palette['colors'][d * 3 + 1], palette['colors'][d * 3 + 2]]
  //     //     return channelColor;
  //     // })
  //     console.log("Old Palette Colors", newdata);
  //     useChannelsStore.setState({ colors: newdata });

  //     let paletteLoss = await calculatePaletteLoss(
  //       channelsVisible,
  //       loader,
  //       selections,
  //       contrastLimits,
  //       colors,
  //       pyramidResolution
  //     );
  //     context.setPaletteLoss(paletteLoss);
  //   };
  const savePalette = () => {
    let palettes = savePastPalette(
      colors,
      channelsVisible,
      contrastLimits,
      selections,
      viewState
    );
    context.setPastPalettes(palettes);
  };
  const clearPalettes = () => {
    let palettes = clearPastPalettes();
    context.setPastPalettes(palettes);
  };
  const deletePalette = (id) => {
    let palettes = deletePastPalette(id);
    context.setPastPalettes(palettes);
  };
  // function savePastPalette(
  //   colors,
  //   channelsVisible,
  //   contrastLimits,
  //   selections,
  //   viewState
  // )

  //   const savePastPalette = async () => {
  //     const visibleIndices = selections
  //       .map((s) => s.c)
  //       .filter((d, i) => channelsVisible[i]);

  //     const fetchUrl = new URL(
  //       `${import.meta.env.VITE_BACKEND_URL}/save_past_palette`
  //     );
  //     const params = {
  //       pathName: context?.randomState?.dataPath,
  //       channelsVisible: JSON.stringify(visibleIndices),
  //       colorSpace: colormap,
  //       colors,
  //       contrastLimits: JSON.stringify(contrastLimits),
  //       optimizationScope:
  //         context.optimizationScope === "global"
  //           ? JSON.stringify({})
  //           : JSON.stringify(makeBoundingBox(viewState)),
  //       z: selections[0].z,
  //     };
  //     fetchUrl.search = new URLSearchParams(params).toString();
  //     const response = await fetch(fetchUrl);
  //     const newColors = response.ok ? await response.json() : {};
  //     newColors["fromHistoryClick"] = true;
  //     context.setPastPalettes([newColors, ...context.pastPalettes]);
  //   };

  //   useEffect(() => {
  //     const palettes = getPastPalettes();
  //     context.setPastPalettes(palettes);
  //   }, []);
  return (
    <ThemeProvider theme={darkTheme}>
      <div style={{ overflowY: "hidden" }}>
        <Grid
          container
          justifyContent="flex-start"
          direction="column"
          m={1}
          //   p={1}
          sx={{ height: "100%", width: "200px" }}
        >
          <Grid
            item
            container
            direction="row"
            justifyContent="flex-start"
            m={1}
            sx={{ borderBottom: "1px solid white" }}
          >
            <Grid item xs={8}>
              <h4 style={{ margin: 0 }}>Palette History</h4>
            </Grid>
            <Grid item xs={2} container justifyContent="center">
              <SaveIcon onClick={savePalette} sx={{ cursor: "pointer" }} />
            </Grid>
            <Grid item xs={2} container justifyContent="center">
              <ClearIcon onClick={clearPalettes} sx={{ cursor: "pointer" }} />
            </Grid>
          </Grid>

          {context?.pastPalettes?.map((palette, index) => {
            return (
              <Grid item container key={palette["id"]} direction="row">
                <Grid
                  item
                  xs={12}
                  container
                  direction="row"
                  justifyContent="space-between"
                  alignItems="center"
                >
                  <MenuItem
                    onClick={() => {
                      selectOldPalette(palette);
                    }}
                  >
                    {palette["channelsVisible"].map((d, index) => {
                      if (d) {
                        return (
                          <div
                            key={palette["id"] + "_" + index}
                            style={{
                              margin: "2px",
                              width: "25px",
                              height: "25px",
                              backgroundColor: `rgb(${palette["colors"][index][0]},${palette["colors"][index][1]},${palette["colors"][index][2]})`,
                              borderRadius: "5px",
                            }}
                          />
                        );
                      }
                    })}
                    <Grid item style={{ flexGrow: 1 }}></Grid>
                  </MenuItem>
                  <MenuItem
                    onClick={() => {
                      deletePalette(palette["id"]);
                    }}
                  >
                    <Grid item>
                      <RemoveIcon />
                    </Grid>
                  </MenuItem>
                </Grid>
              </Grid>
            );
          })}
        </Grid>
      </div>
    </ThemeProvider>
  );
};

export default PastPalettes;
