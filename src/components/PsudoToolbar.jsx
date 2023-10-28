import React, { useContext, useEffect, useState } from "react";
import { AppContext } from "../context/GlobalContext.jsx";
import ColorNameSelect from "./ColorNameSelect.jsx";
import AddPhotoAlternateIcon from "@mui/icons-material/AddPhotoAlternate";
import {
  Grid,
  IconButton,
  Menu,
  TextField,
  Typography,
  Icon,
  FormControl,
  FormLabel,
  RadioGroup,
  FormControlLabel,
  Radio,
  Button,
} from "@mui/material";
import { createTheme, ThemeProvider } from "@mui/material/styles";
import {
  useChannelsStore,
  useViewerStore,
  useImageSettingsStore,
} from "../Avivator/state.js";
import { getNameFromUrl } from "../Avivator/viewerUtils.js";
import HistoryIcon from "@mui/icons-material/History";
import PastPalettes from "./PastPalettes.jsx";
import JoinInnerIcon from "@mui/icons-material/JoinInner";
import BarChartIcon from "@mui/icons-material/BarChart";

import shallow from "zustand/shallow";
import SingleChannelWrapper from "./SingleChannelWrapper.jsx";
import { AttachFile, InsertLink } from "@mui/icons-material";
import DropzoneButton from "../Avivator/components/Controller/components/DropzoneButton.jsx";
import Footer from "../Avivator/components/Footer.jsx";
import AddChannel from "../Avivator/components/Controller/components/AddChannel.jsx";
import _ from 'lodash';

function PsudoToolbar() {
  const context = useContext(AppContext);
  const [anchorElSource, setAnchorElSource] = useState(null);
  const openSource = Boolean(anchorElSource);
  const [anchorElHistory, setAnchorElHistory] = useState(null);
  const openHistory = Boolean(anchorElHistory);
  const [anchorElOverlap, setAnchorElOverlap] = useState(null);
  const openOverlap = Boolean(anchorElOverlap);
  const [anchorElOptimization, setAnchorElOptimization] = useState(null);
  const openOptimization = Boolean(anchorElOptimization);
  const [anchorElChart, setAnchorElChart] = useState(null);

  const openChart = Boolean(anchorElChart);

  const [lensEnabled] = useImageSettingsStore(
    (store) => [store.lensEnabled],
    shallow
  );

  const [showFileUpload, setShowFileUpload] = useState(false);
  const [showLinkInput, setShowLinkInput] = useState(false);
  const [source] = useViewerStore((store) => [store.source], shallow);
  const [imageUrl, setImageUrl] = useState(source?.urlOrFile);
  let [channelsVisible] = useChannelsStore(
    (store) => [store.channelsVisible],
    shallow
  );
  useEffect(() => {
    setImageUrl(source?.urlOrFile);
  }, [source]);

  const handleFileButtonClick = () => {
    setShowFileUpload(true);
    setShowLinkInput(false);
  };

  const handleLinkButtonClick = () => {
    setShowFileUpload(false);
    setShowLinkInput(true);
  };

  const handleSourceClick = (event) => {
    console.log(source);
    setAnchorElSource(event.currentTarget);
  };

  const handleCloseSource = () => {
    setAnchorElSource(null);
  };

  const handleCloseOptimization = () => {
    setAnchorElOptimization(null);
  };
  const handleOverlapClick = (event) => {
    setAnchorElOverlap(event.currentTarget);
  };

  const handleCloseOverlap = () => {
    setAnchorElOverlap(null);
  };
  const handleChartClick = (event) => {
    console.log("handleChartClick", openChart);
    setAnchorElChart(event.currentTarget);
  };

  const handleCloseChart = () => {
    setAnchorElChart(null);
  };
  const handleOptimizationClick = (event) => {
    setAnchorElOptimization(event.currentTarget);
  };

  const handleHistoryClick = (event) => {
    setAnchorElHistory(event.currentTarget);
  };

  const handleCloseHistory = () => {
    setAnchorElHistory(null);
  };
  const darkTheme = createTheme({ palette: { mode: "dark" } });

  const updateUrl = (e) => {
    console.log(e, "e");
    setImageUrl(e.target.value);
  };

  const submitUrl = (e) => {
    if (e.key === "Enter") {
      updateSourceUrl();
    }
  };

  const updateSourceUrl = () => {
    useViewerStore.setState({
      source: {
        urlOrFile: imageUrl,
        description: getNameFromUrl(imageUrl),
      },
    });
  };
  const toggleShowLens = () => {
    useImageSettingsStore.setState({ lensEnabled: !_.cloneDeep(lensEnabled) });
  };

  const optimize = async () => {
    const channelsPayload = [];
    channelsVisible.forEach((d, i) => {
      if (d) {
        const channelPayload = {
          color: colors[i],
          contrastLimits: contrastLimits[i],
          selection: selections[i],
        };
        channelsPayload.push(channelPayload);
      }
    });

    let intensityList = new Float32Array([]);
    let colorList = new Uint16Array([]);
    await Promise.all(
      channelsPayload.map(async (d, i) => {
        // const resolution = pyramidResolution;
        const raster = await loader?.[pyramidResolution]?.getRaster({
          selection: d.selection,
        });
        // Print the size of raster.data
        console.log("raster.data size", raster.data.length, raster.data.length);
        // Subsample
        // time the next line
        console.time("channel_gmm");
        const conrastLimits = psudoAnalysis.channel_gmm(raster.data);
        console.timeEnd("channel_gmm");
        let floatArr = new Float32Array([...conrastLimits]);
        // Make Javascript Array
        floatArr = Array.from(floatArr);
        console.log("conrastLimits", floatArr);
      })
    );

    // Length of each channel's intensity list.
    const channelLength = intensityList.length / channelsPayload.length;

    console.log("intensityList", channelLength, intensityList);

    // New intensityList
    const newIntensityList = new Float32Array(intensityList.length);
    console.time("Code execution time");

    // Iterate for channelLength
    _.range(channelLength).map((i) => {
      _.range(channelsPayload.length).map((j) => {
        newIntensityList[i * channelsPayload.length + j] =
          intensityList[i + j * channelLength];
      });
    });

    console.timeEnd("Code execution time");

    console.log(
      "newIntensityList",
      JSON.stringify(Array.from(newIntensityList))
    );

    // convert Uint16Array to Float32 Array, where each value is /255
    const colorListFloat = new Float32Array(colorList.length);
    colorList.forEach((d, i) => {
      colorListFloat[i] = d / 255;
    });
    console.log("colors", colorList, colorListFloat);
    // const test = psudoAnalysis.optimize_palette(intensityList, colorList);
    // console.log(`Call to doSomething took ${performance.now() - startTime} milliseconds`, test)
    // console.log('test', JSON.stringify({
    //     'intensityList': Array.from(intensityList),
    //     'colorList': Array.from(colorList)
    // }))
    //
    //
    // psudoAnalysis.optimize_palette(newIntensityList).then(result => {
    //     console.log('TESULT', JSON.stringify(Array.from(result)));
    // });/**/
    // psudoAnalysis.optimize_palette(newIntensityList).then((result) => {
    //   console.log("TESULT", result);
    // }); /**/
    let startTime = performance.now();

    // let tes = psudoAnalysis.optimize_palette(newIntensityList);
    // console.log(tes);
    // console.log(`Call to doSomething took ${performance.now() - startTime} milliseconds`)
    // // console.log(`Call to doSomething took ${performance.now() - startTime} milliseconds`)
    //
    // //
    //
    // _.range(10).map(() => {
    //
    //         const test = psudoAnalysis.color_test(new Float32Array([0.5, 0.2]));
    //         console.log(`Call to doSomething took ${performance.now() - startTime} milliseconds`)
    //     }
    // )
    //
    // // console.log('test', JSON.stringify({'arr': Array.from(test)}))
    // // console.log('Adding one', psudoAnalysis.add_one(444))
    // // console.log('uint16Colors', uint16Colors, uint16ContrastLimits, uint16ChannelRaster)
  };
  const handleChangeOptimizationScope = (event) => {
    context.setOptimizationScope(event.target.value);
  };
  return (
    <ThemeProvider theme={darkTheme}>
      <Grid
        container
        sx={{ top: 0, left: 0, position: "absolute" }}
        direction="row"
        justifyContent="flex-start"
        alignItems="center"
      >
        <Grid item xs={"auto"} p={1}>
          <h1 className={"title-code color-gradient"}>psudo</h1>
        </Grid>

        <Grid item xs={"auto"} sx={{ zIndex: 10000 }}>
          <IconButton
            id="history-button"
            aria-controls={openHistory ? "basic-menu" : undefined}
            aria-haspopup="true"
            aria-expanded={openHistory ? "true" : undefined}
            onClick={handleHistoryClick}
          >
            <HistoryIcon />
          </IconButton>
          <Menu
            id="basic-menu"
            anchorEl={anchorElHistory}
            open={openHistory}
            onClose={handleCloseHistory}
            MenuListProps={{
              "aria-labelledby": "basic-button",
            }}
            sx={{ marginTop: 5 }}
          >
            <PastPalettes />
          </Menu>
        </Grid>
      </Grid>

      <Grid
        container
        sx={{ bottom: 0, left: 0, position: "absolute", zIndex: 100000 }}
        direction="row"
        justifyContent="space-between"
        // alignItems="center"
      >
        <Grid item xs={"auto"}>
          <IconButton
            id="source-button"
            aria-controls={openSource ? "basic-menu" : undefined}
            aria-haspopup="true"
            aria-expanded={openSource ? "true" : undefined}
            onClick={handleOverlapClick}
          >
            <JoinInnerIcon sx={{ fontSize: 40 }} />
          </IconButton>
          <Menu
            id="basic-menu"
            anchorEl={anchorElOverlap}
            open={openOverlap}
            onClose={handleCloseOverlap}
            MenuListProps={{
              "aria-labelledby": "basic-button",
            }}
          >
            <Grid
              container
              direction="column"
              justifyContent="center"
              alignItems="center"
              p={0}
              m={0}
            >
              <Grid item p={0} m={0}>
                <h4 style={{ margin: 0, padding: 0 }}>Channel Overlap</h4>
              </Grid>

              <Grid item p={0} m={0}>
                <SingleChannelWrapper
                  label={"Channel Overlap"}
                  channelIndex={-1}
                  width={128}
                  height={128}
                  absoluteScale={1}
                  channelsVisible={channelsVisible}
                ></SingleChannelWrapper>
              </Grid>
            </Grid>
          </Menu>
          <Footer />
        </Grid>
        <Grid item xs={"auto"}>
          <IconButton onClick={toggleShowLens}>
            <Icon sx={{ fontSize: 30 }}>
              <img
                src={
                  lensEnabled
                    ? "/src/assets/add-lens-icon.svg"
                    : "/src/assets/remove-lens-icon.svg"
                }
              />
            </Icon>
          </IconButton>
        </Grid>
        <Grid item xs={"auto"}>
          <IconButton
            id="source-button"
            aria-controls={openChart ? "basic-menu" : undefined}
            aria-haspopup="true"
            aria-expanded={openChart ? "true" : undefined}
            onClick={handleChartClick}
          >
            <BarChartIcon sx={{ fontSize: 40 }} />
          </IconButton>
          <Menu
            id="basic-menu"
            anchorEl={anchorElChart}
            open={openChart}
            onClose={handleCloseChart}
            MenuListProps={{
              "aria-labelledby": "basic-button",
            }}
          >
            <Grid
              container
              direction="column"
              justifyContent="center"
              alignItems="center"
              p={0}
              m={0}
            >
              <Grid item p={0} m={0}>
                <h4 style={{ margin: 0, padding: 0 }}>Chart</h4>
              </Grid>
            </Grid>
          </Menu>
          <Footer />
        </Grid>
        <Grid item xs={"auto"}>
          {/* Increase size of icon */}
          <IconButton
            id="optimizationButton-button"
            aria-controls={openOptimization ? "basic-menu" : undefined}
            aria-haspopup="true"
            aria-expanded={openOptimization ? "true" : undefined}
            onClick={handleOptimizationClick}
            style={{ borderRadius: 10 }}
          >
            <Icon sx={{ fontSize: 40 }}>
              <img src="/src/assets/color-palette-icon.svg" />
            </Icon>
          </IconButton>
          <Menu
            id="basic-menu"
            anchorEl={anchorElOptimization}
            open={openOptimization}
            onClose={handleCloseOptimization}
            MenuListProps={{
              "aria-labelledby": "basic-button",
            }}
            sx={{ zIndex: "100000000" }}
          >
            <Grid
              container
              direction="column"
              justifyContent="center"
              alignItems="center"
              p={0}
              sx={{ margin: "20px 0" }}
            >
              <Grid item xs={12} p={1} sx={{ width: "100%" }}>
                <ColorNameSelect label={"Excluded Colors"} multiSelect={true} />
              </Grid>

              <Grid item xs={12} p={1}>
                <FormControl>
                  <FormLabel id="optimization-scope-grouplabel">
                    Optimization Scope
                  </FormLabel>
                  <RadioGroup
                    row
                    aria-labelledby="optimization-scope-grouplabel"
                    value={context.optimizationScope}
                    onChange={handleChangeOptimizationScope}
                  >
                    <FormControlLabel
                      value="global"
                      control={<Radio />}
                      label="Global"
                    />
                    <FormControlLabel
                      value="viewport"
                      control={<Radio />}
                      label="Viewport"
                    />
                  </RadioGroup>
                </FormControl>
              </Grid>
              <Grid item xs={12}>
                <Button variant="contained" onClick={optimize}>
                  Optimize Palette
                </Button>
              </Grid>
            </Grid>
          </Menu>
        </Grid>
      </Grid>
      <Grid
        container
        sx={{ top: 0, right: 0, position: "absolute" }}
        direction="row"
        justifyContent="flex-end"
        alignItems="center"
        p={1}
      >
        <Grid item xs={5} style={{ zIndex: 100 }} alignItems={"center"}>
          <AddChannel />
        </Grid>
        <Grid item xs={"auto"} style={{ zIndex: 100 }} alignItems={"center"}>
          <IconButton
            id="source-button"
            aria-controls={openSource ? "basic-menu" : undefined}
            aria-haspopup="true"
            aria-expanded={openSource ? "true" : undefined}
            onClick={handleSourceClick}
            style={{ borderRadius: 10 }}
          >
            <span style={{ fontSize: "1rem" }}>Source&nbsp;</span>
            <AddPhotoAlternateIcon />
          </IconButton>
          <Menu
            id="basic-menu"
            anchorEl={anchorElSource}
            open={openSource}
            onClose={handleCloseSource}
            MenuListProps={{
              "aria-labelledby": "basic-button",
            }}
            style={{ marginTop: 5, width: 350 }}
          >
            <Grid
              container
              height={60}
              direction="row"
              justifyContent="space-around"
              alignItems="center"
              style={{ width: showLinkInput || showFileUpload ? 300 : 150 }}
            >
              <Grid item xs={"auto"} align="center">
                <IconButton
                  onClick={handleLinkButtonClick}
                  color={showLinkInput ? "primary" : "white"}
                >
                  <InsertLink />
                </IconButton>
                <Typography
                  variant="subtitle2"
                  align="center"
                  style={{
                    transition: "all 0s linear 0s",
                    display: showLinkInput || showFileUpload ? "none" : "block",
                  }}
                >
                  Link
                </Typography>
              </Grid>
              <Grid
                item
                xs={"auto"}
                m={0}
                p={0}
                align="center"
                style={{
                  transition: "width 0s ease-in",
                  width: showLinkInput || showFileUpload ? "200px" : "0px",
                }}
              >
                {(showLinkInput || showFileUpload) && (
                  <>
                    {showLinkInput && (
                      <TextField
                        id="link-input"
                        variant="outlined"
                        margin="dense"
                        size="small"
                        fullWidth
                        value={imageUrl}
                        onChange={updateUrl}
                        onKeyDown={submitUrl}
                      />
                    )}
                    {showFileUpload && <DropzoneButton />}
                  </>
                )}
              </Grid>
              <Grid item xs={"auto"} align="center">
                <IconButton
                  onClick={handleFileButtonClick}
                  color={showFileUpload ? "primary" : "white"}
                >
                  <AttachFile />
                </IconButton>
                <Typography
                  variant="subtitle2"
                  align="center"
                  style={{
                    transition: "all 0s linear 0s",
                    display: showLinkInput || showFileUpload ? "none" : "block",
                  }}
                >
                  File
                </Typography>
              </Grid>
            </Grid>
          </Menu>
        </Grid>
      </Grid>
    </ThemeProvider>
  );
}

export default PsudoToolbar;
