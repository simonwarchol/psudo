import React, { useContext, useEffect, useRef, useState } from "react";
import { AppContext } from "../context/GlobalContext.jsx";
import {
  Button,
  Grid,
  IconButton,
  Menu,
  Select,
  Slider,
  Icon,
} from "@mui/material";
import { ChromePicker } from "react-color";
import LockOpenIcon from "@mui/icons-material/LockOpen";
import ContrastIcon from "@mui/icons-material/Contrast";
import VisibilityIcon from "@mui/icons-material/Visibility";
import SquareIcon from "@mui/icons-material/Square";

import VisibilityOffIcon from "@mui/icons-material/VisibilityOff";
import ColorNameSelect from "./ColorNameSelect.jsx";
import LockIcon from "@mui/icons-material/Lock";
import Typography from "@mui/material/Typography";
import {
  useChannelsStore,
  useLoader,
  useViewerStore,
  useImageSettingsStore,
} from "../Avivator/state.js";
import shallow from "zustand/shallow";

import _ from "lodash";
import {
  getSingleSelectionStats,
  truncateDecimalNumber,
  getGMMContrastLimits,
  calculatePaletteLoss,
  calculateConfusionLoss,
} from "../Avivator/viewerUtils.js";

// import lodash
function ChannelColorDisplay(props) {
  const { channelIndex } = props;
  const context = useContext(AppContext);
  const [pickerColor, setPickerColor] = useState({});
  const [showPicker, setShowPicker] = useState(false);
  const [colorLocked, setColorLocked] = useState(false);
  const [
    channelsVisible,
    contrastLimits,
    colors,
    domains,
    selections,
    ids,
    setPropertiesForChannel,
    toggleIsOnSetter,
    removeChannel,
  ] = useChannelsStore(
    (store) => [
      store.channelsVisible,
      store.contrastLimits,
      store.colors,
      store.domains,
      store.selections,
      store.ids,
      store.setPropertiesForChannel,
      store.toggleIsOn,
      store.removeChannel,
    ],
    shallow
  );
  const [
    source,
    channelOptions,
    useColormap,
    isChannelLoading,
    setIsChannelLoading,
    removeIsChannelLoading,
    pixelValues,
    isViewerLoading,
    pyramidResolution,
  ] = useViewerStore(
    (store) => [
      store.source,
      store.channelOptions,
      store.useColormap,
      store.isChannelLoading,
      store.setIsChannelLoading,
      store.removeIsChannelLoading,
      store.pixelValues,
      store.isViewerLoading,
      store.pyramidResolution,
    ],
    shallow
  );
  const [lensSelection] = useImageSettingsStore(
    (store) => [store.lensSelection],
    shallow
  );
  const [anchorEl, setAnchorEl] = useState(null);
  const [scale, setScale] = useState({});

  const openEl = Boolean(anchorEl);
  const handleElClick = (event) => {
    setAnchorEl(event.currentTarget);
  };
  const handleCloseEl = () => {
    setAnchorEl(null);
  };

  const ref = useRef(null);
  const paperRef = useRef(null);
  const loader = useLoader();
  useEffect(() => {
    console.log(lensSelection, "lensSelection");
  }, [lensSelection]);

  // useEffect(() => {
  //   console.log("isChannelLoading", isChannelLoading);
  // }, [isChannelLoading]);

  const rgbColor = `rgb(${colors?.[channelIndex]})`;
  let ind = channelIndex;

  const [min, max] = domains[ind];
  // If the min/max range is and the dtype is float, make the step size smaller so contrastLimits are smoother.
  const { dtype } = loader[0];
  const isFloat = dtype === "Float32" || dtype === "Float64";
  const step = max - min < 500 && isFloat ? (max - min) / 500 : 1;

  const handleSliderChange = (e, v) => {
    setPropertiesForChannel(ind, { contrastLimits: v });
  };
  const handleRemoveChannel = () => {
    removeChannel(channelIndex);
    removeIsChannelLoading(channelIndex);
  };

  useEffect(() => {
    if (colors?.[channelIndex]) {
      const color = colors?.[channelIndex];
      let rgbColor = { r: color?.[0], g: color?.[1], b: color?.[2] };
      setPickerColor(rgbColor);
    }
  }, [colors]);

  const toggleVisibility = () => {
    let _tmpChannelsVisible = _.cloneDeep(channelsVisible);
    _tmpChannelsVisible[channelIndex] = !_tmpChannelsVisible[channelIndex];
    console.log("_tmpChannelsVisible", _tmpChannelsVisible);
    useChannelsStore.setState({
      channelsVisible: _tmpChannelsVisible,
    });
  };

  const calculateContrastLimits = () => {
    context?.setIsLoading(true);
    getGMMContrastLimits({
      loader,
      selection: selections[channelIndex],
      pyramidResolution,
    })
      .then((intContrastLimits) => {
        setPropertiesForChannel(ind, { contrastLimits: intContrastLimits });
      })
      .finally(() => {
        context?.setIsLoading(false);
      });
  };

  const addRemoveToLens = () => {
    let lensSelectionCopy = _.cloneDeep(lensSelection);
    lensSelectionCopy[channelIndex] = !lensSelectionCopy[channelIndex] ? 1 : 0;
    useImageSettingsStore.setState({ lensSelection: lensSelectionCopy });
  };

  function colorArrayToRGB(color) {
    return `rgb(${color[0]}, ${color[1]}, ${color[2]})`;
  }

  const handleChange = async (color, e) => {
    setPickerColor(color.rgb);
    colors[channelIndex] = [pickerColor.r, pickerColor.g, pickerColor.b];
    if (_.isEmpty(colors) || _.isEmpty(context?.graphData)) return;
    let tmpGraphData = [];
    let ii = 0;
    for (const [i, visible] of (channelsVisible || []).entries()) {
      if (visible) {
        let channelGraphData = context?.graphData?.[ii] || {};
        channelGraphData.color = colors[i];
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
      context?.luminanceValue
    );

    context?.setPaletteLoss(paletteLoss);

    useChannelsStore.setState({ colors: colors, prevColors: colors });
  };

  const displayValueFunction = (v) => {
    if (scale && scale?.shift && scale?.scale) {
      v = (v / 65535) * scale?.scale + scale?.shift;
    }
    return truncateDecimalNumber(v, 5);
  };

  const getPickerLocation = () => {
    const bottom = ref?.current?.getBoundingClientRect()?.bottom;
    const clientHeight = document.body.clientHeight;
    return bottom;
  };

  const toggleLock = () => {
    let _lockedChannelColors = _.cloneDeep(context.lockedChannelColors);
    _lockedChannelColors[channelIndex] = !colorLocked;
    context.setLockedChannelColors(_lockedChannelColors);
    setColorLocked(!colorLocked);
  };

  const onSelectionChange = (e) => {
    console.log("selection change");
    const selection = {
      ...selections[ind],
      c: channelOptions.indexOf(e.target.value),
    };
    setIsChannelLoading(ind, true);
    getSingleSelectionStats({
      loader,
      selection,
      pyramidResolution,
      channelVisible: channelsVisible[ind],
    }).then(({ domain, contrastLimits: newContrastLimit }) => {
      setPropertiesForChannel(ind, {
        contrastLimits: newContrastLimit,
        domains: domain,
      });
      setIsChannelLoading(ind, false);
      setPropertiesForChannel(ind, { selections: selection });
    });
  };

  return (
    <>
      {colors?.[channelIndex] !== undefined && (
        <>
          <Grid
            container
            sx={{
              height: "100%",
              opacity: channelsVisible?.[channelIndex] ? 1 : 0.5,
            }}
            direction="row"
            justifyContent="space-evenly"
            alignItems="center"
            // If !channelsVisible?.[channelIndex], set opacity to 0.5

            ref={ref}
          >
            <Grid
              item
              xs={1}
              sx={{
                display: "flex",
                alignItems: "center",
                justifyContent: "center",
              }}
            >
              <SquareIcon
                onClick={handleElClick}
                sx={{
                  fontSize: 35, // or your preferred icon size
                  color: colorArrayToRGB(colors[channelIndex]), // This sets the color of the icon
                  cursor: "pointer", // change cursor style during hover
                  ":hover": {
                    color: colorArrayToRGB(colors[channelIndex]), // Keep the same color on hover, or choose another
                  },
                }}
              />
              <Menu
                id="basic-menu"
                anchorEl={anchorEl}
                open={openEl}
                onClose={handleCloseEl}
                MenuListProps={{
                  "aria-labelledby": "basic-button",
                }}
              >
                <ColorNameSelect
                  label={"Color Name"}
                  width={"100%"}
                  channelIndex={channelIndex}
                  multiSelect={false}
                />
                <ChromePicker
                  disableAlpha={true}
                  color={pickerColor}
                  onChange={handleChange}
                />
              </Menu>
            </Grid>
            <Grid item xs={4}>
              <Select
                native
                value={channelOptions[selections[ind].c]}
                onChange={onSelectionChange}
                variant="standard"
              >
                {channelOptions.map((opt) => (
                  <option key={opt} value={opt}>
                    {opt}
                  </option>
                ))}
              </Select>
            </Grid>
            <Grid item xs={1}>
              <IconButton color="white" onClick={toggleVisibility}>
                {channelsVisible?.[channelIndex] && <VisibilityIcon />}
                {!channelsVisible?.[channelIndex] && <VisibilityOffIcon />}
              </IconButton>
            </Grid>
            <Grid item xs={1}>
              <IconButton color="white" onClick={toggleLock}>
                {colorLocked && <LockIcon />}
                {!colorLocked && <LockOpenIcon />}
              </IconButton>
            </Grid>
            <Grid item xs={1}>
              <IconButton color="white" onClick={calculateContrastLimits}>
                <Icon sx={{ fontSize: 30 }}>
                  <img src="/auto-contrast-icon.svg" />
                </Icon>
              </IconButton>
            </Grid>
            <Grid item xs={1}>
              <IconButton color="white" onClick={addRemoveToLens}>
                <Icon sx={{ fontSize: 30 }}>
                  <img
                    src={
                      lensSelection[channelIndex] == 0
                        ? "/add-to-lens-icon.svg"
                        : "/remove-from-lens-icon.svg"
                    }
                  />
                </Icon>
              </IconButton>
            </Grid>
            {/* <Grid item xs={4} sx={{ paddingLeft: "15px" }}>
              
            </Grid> */}
            {channelsVisible?.[channelIndex] && (
              <Grid item xs={8}>
                <Slider
                  value={contrastLimits[ind]}
                  onChange={handleSliderChange}
                  valueLabelDisplay="auto"
                  valueLabelFormat={(v) => displayValueFunction(v)}
                  min={min}
                  max={max}
                  step={step}
                  orientation="horizontal"
                  style={{
                    color: rgbColor,
                    //   margin: "0 20px",
                  }}
                />
              </Grid>
            )}
          </Grid>
        </>
      )}
    </>
  );
}

export default ChannelColorDisplay;
