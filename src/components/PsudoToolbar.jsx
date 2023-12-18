import React, { useContext, useEffect, useState } from "react";
import { AppContext } from "../context/GlobalContext.jsx";
import ColorNameSelect from "./ColorNameSelect.jsx";
import AddPhotoAlternateIcon from "@mui/icons-material/AddPhotoAlternate";
import * as psudoAnalysis from "psudo-analysis";

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
  Slider,
} from "@mui/material";
import { createTheme, ThemeProvider } from "@mui/material/styles";
import {
  useChannelsStore,
  useViewerStore,
  useImageSettingsStore,
  useLoader,
} from "../Avivator/state.js";
import {
  getNameFromUrl,
  calculatePaletteLoss,
  getGMMContrastLimits,
  createContiguousArrays,
  getChannelPayload,
  getLensIntensityValues,
} from "../Avivator/viewerUtils.js";
import HistoryIcon from "@mui/icons-material/History";
import PastPalettes from "./PastPalettes.jsx";
import JoinInnerIcon from "@mui/icons-material/JoinInner";
import AutoGraphIcon from "@mui/icons-material/AutoGraph";
import LinkedCameraIcon from "@mui/icons-material/LinkedCamera";
import shallow from "zustand/shallow";
import SingleChannelWrapper from "./SingleChannelWrapper.jsx";
import { AttachFile, InsertLink } from "@mui/icons-material";
import DropzoneButton from "../Avivator/components/Controller/components/DropzoneButton.jsx";
import Footer from "../Avivator/components/Footer.jsx";
import AddChannel from "../Avivator/components/Controller/components/AddChannel.jsx";
import { VegaLite } from "react-vega";
import LineChart from "./LineChart.jsx";
import _ from "lodash";
import GaugeCharts from "./GaugeCharts.jsx";

function PsudoToolbar() {
  const context = useContext(AppContext);
  const [anchorElSource, setAnchorElSource] = useState(null);
  const openSource = Boolean(anchorElSource);
  const [anchorElHistory, setAnchorElHistory] = useState(null);
  const openHistory = Boolean(anchorElHistory);
  const [anchorElOverlap, setAnchorElOverlap] = useState(null);
  const [anchorElOptimization, setAnchorElOptimization] = useState(null);
  const openOptimization = Boolean(anchorElOptimization);
  const [anchorElChart, setAnchorElChart] = useState(null);

  const openChart = Boolean(anchorElChart);

  const [lensEnabled] = useImageSettingsStore(
    (store) => [store.lensEnabled],
    shallow
  );
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

  const [showFileUpload, setShowFileUpload] = useState(false);
  const [showLinkInput, setShowLinkInput] = useState(false);
  const [imageUrl, setImageUrl] = useState(source?.urlOrFile);
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

  // function luminanceValueText(value) {
  //   // round to 2 decimal places
  //   console.log("val", value);
  //   return `${(value / 100.0).toFixed(2)}`;
  // }

  const handleLuminanceChange = (event, newValue) => {
    context.setLuminanceValue(newValue);
  };
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

  const toggleOverlapView = () => {
    context?.setOverlapView(!_.cloneDeep(context?.overlapView));
  };

  const toggleLinkedViews = () => {
    context?.setLinkedViews(!_.cloneDeep(context?.linkedViews));
  };

  const spec = {
    width: 400,
    height: 200,
    mark: "bar",
    encoding: {
      x: { field: "a", type: "ordinal" },
      y: { field: "b", type: "quantitative" },
    },
    data: { name: "table" }, // note: vega-lite data attribute is a plain object instead of an array
  };

  const barData = {
    table: [
      { a: "A", b: 28 },
      { a: "B", b: 55 },
      { a: "C", b: 43 },
      { a: "D", b: 91 },
      { a: "E", b: 81 },
      { a: "F", b: 53 },
      { a: "G", b: 19 },
      { a: "H", b: 87 },
      { a: "I", b: 52 },
    ],
  };

  const optimize = async () => {
    context?.setIsLoading(true);
    console.log("lcc", context?.lockedChannelColors);
    const channelsPayload = [];
    let payload;
    if (lensEnabled && context.optimizationScope == "lens") {
      payload = await getLensIntensityValues(
        context?.coordinate,
        viewState,
        loader,
        pyramidResolution,
        lensRadius,
        channelsVisible,
        selections,
        setMovingLens,
        contrastLimits,
        colors
      );
    } else {
      payload = await getChannelPayload(
        channelsVisible,
        colors,
        selections,
        contrastLimits,
        loader,
        pyramidResolution,
        10000
      );
    }

    const { intensityArray, colorArray, contrastLimitsArray } =
      createContiguousArrays(payload);
    console.log(
      "intensityArray",
      intensityArray,
      colorArray,
      contrastLimitsArray
    );

    // Iterate over colorList // 3 length
    let lockedList = [];
    for (let i = 0; i < colorArray.length / 3; i += 1) {
      if (context?.lockedChannelColors[i]) {
        lockedList.push(1);
      } else {
        lockedList.push(0);
      }
    }

    // console.log("LumVal", context.luminanceValue);

    const optColors = psudoAnalysis.optimize(
      colorArray,
      lockedList,
      intensityArray,
      contrastLimitsArray,
      context?.luminanceValue,
      context?.colorExcluded
    );
    console.log("Post Call Opt colors", optColors);
    console.log("Colors Before", colors);
    let colorCounter = 0;
    let tmpColors = _.cloneDeep(colors);
    channelsVisible.forEach((d, i) => {
      if (d) {
        tmpColors[i] = Array.from(
          optColors.slice(colorCounter, colorCounter + 3).map((d) => d * 255)
        );
        colorCounter += 3;
      }
    });
    useChannelsStore.setState({ colors: tmpColors, prevColors: tmpColors });

    let tmpGraphData = [];
    let ii = 0;
    for (const [i, visible] of (channelsVisible || []).entries()) {
      if (visible) {
        let channelGraphData = context?.graphData[ii];
        channelGraphData.color = tmpColors[i];
        tmpGraphData.push(channelGraphData);
        ii++;
      }
    }
    context?.setGraphData(tmpGraphData);

    let paletteLoss = await calculatePaletteLoss(
      channelsVisible,
      loader,
      selections,
      contrastLimits,
      colors,
      pyramidResolution,
      context?.luminanceValue,
      context?.colorExcluded
    );

    context?.setPaletteLoss(paletteLoss);

    context?.setIsLoading(false);
  };

  const handleChangeOptimizationScope = (event) => {
    context.setOptimizationScope(event.target.value);
  };
  return (
    <ThemeProvider theme={darkTheme}>
      <Grid
        container
        sx={{ top: 0, right: 0, position: "absolute" }}
        direction="column"
        justifyContent="flex-end"
        alignItems="start"
        // p={1}
      >
        <Grid item sx={{ marginTop: 20, zIndex: 10000 }}>
          <GaugeCharts
            paletteLoss={context?.paletteLoss}
            style={{ position: "relative", top: "50%", zIndex: 10000 }}
          />
        </Grid>
      </Grid>
      <Grid
        container
        sx={{ top: 0, left: 0, position: "absolute" }}
        direction="row"
        justifyContent="flex-start"
        alignItems="center"
        // p={1}
      >
        <Grid
          item
          xs={"auto"}
          p={1}
          container
          direction="column"
          sx={{ zIndex: 10000, display: "flex", alignItems: "center" }}
        ></Grid>

        <Grid
          container
          item
          xs={"auto"}
          sx={{ zIndex: 10000, display: "flex", alignItems: "flex-end" }} // Updated alignItems to 'flex-end'
        ></Grid>

        <Grid item xs={"auto"} p={0} m={0} sx={{ zIndex: 10000 }}>
          <Grid
            container
            direction="column"
            justifyContent="start"
            alignItems="center"
            item
            p={0}
            m={0}
            sx={{ width: 370, height: 80 }}
          >
            <LineChart style={{ position: "relative", left: "50%" }} />
          </Grid>

          <Footer />
        </Grid>
        <Grid item sx={{ marginLeft: 20 }}>
          <h1 className={"title-code color-gradient"}>psudo</h1>
        </Grid>
      </Grid>

      <Grid
        container
        sx={{
          bottom: 0,
          left: 0,
          position: "absolute",
          zIndex: 100000,
          marginBottom: "10px",
        }}
        direction="row"
        justifyContent="start"
        alignItems="center"
      >
        <Grid item xs={"auto"}>
          <IconButton id="overlap-button" onClick={toggleOverlapView}>
            {/* if context?.showOverlap, show at full opacity, otherwise at 0.5 opacity */}
            <JoinInnerIcon
              sx={{ fontSize: 40 }}
              color={context?.overlapView ? "primary" : "white"}
            />
          </IconButton>
        </Grid>
        <Grid item xs={"auto"}>
          <IconButton id="linked-views-button" onClick={toggleLinkedViews}>
            {/* if context?.showOverlap, show at full opacity, otherwise at 0.5 opacity */}
            <LinkedCameraIcon
              sx={{ fontSize: 40 }}
              color={context?.linkedViews ? "primary" : "white"}
            />
          </IconButton>
        </Grid>
        <Grid item xs={"auto"}>
          <IconButton onClick={toggleShowLens}>
            <Icon sx={{ fontSize: 30 }}>
              <img
                src={
                  lensEnabled ? "/add-lens-icon.svg" : "/remove-lens-icon.svg"
                }
              />
            </Icon>
          </IconButton>
        </Grid>
        <Grid item xs={"auto"}>
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
            sx={{ zIndex: 100000000000, marginTop: -10 }}
          >
            <PastPalettes />
          </Menu>
        </Grid>
        {/* Fill max spac */}
        <Grid item sx={{ flexGrow: 1 }}></Grid>
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
              <img src="/color-palette-icon.svg" />
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
              {/* Add a 0-1 slider called luminance range */}
              <Grid item xs={12} p={1}>
                <FormControl>
                  <FormLabel id="optimization-scope-grouplabel">
                    Luminance Range
                  </FormLabel>
                  <Slider
                    getAriaLabel={() => "Luminance Range"}
                    value={context?.luminanceValue}
                    onChange={handleLuminanceChange}
                    valueLabelDisplay="auto"
                    getAriaValueText={(value) => `${value}`}
                  />
                </FormControl>
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
                      value="lens"
                      control={<Radio />}
                      label="Lens"
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

        <Grid
          item
          xs={"auto"}
          style={{ zIndex: 100 }}
          alignItems={"center"}
        ></Grid>
      </Grid>
    </ThemeProvider>
  );
}

export default PsudoToolbar;
