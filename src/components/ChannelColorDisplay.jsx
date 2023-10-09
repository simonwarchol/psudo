import React, { useContext, useEffect, useRef, useState } from "react";
import { AppContext } from "../context/GlobalContext.jsx";
import { Button, Grid, IconButton, Menu, Select, Slider } from "@mui/material";
import { ChromePicker } from "react-color";
import LockOpenIcon from "@mui/icons-material/LockOpen";
import ContrastIcon from "@mui/icons-material/Contrast";
import VisibilityIcon from "@mui/icons-material/Visibility";
import VisibilityOffIcon from "@mui/icons-material/VisibilityOff";
import ColorNameSelect from "./ColorNameSelect.jsx";
import LockIcon from "@mui/icons-material/Lock";
import {
  useChannelsStore,
  useLoader,
  useViewerStore,
} from "../Avivator/state.js";
import shallow from "zustand/shallow";

import _ from "lodash";
import {
  getSingleSelectionStats,
  truncateDecimalNumber,
} from "../Avivator/viewerUtils.js";
import * as psudoAnalysis from "psudo-analysis";

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

  const rgbColor = `rgb(${colors?.[channelIndex]})`;
  let ind = 0;
  for (let i = 0; i < channelIndex; i++) {
    if (channelsVisible[i]) {
      ind++;
    }
  }

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

  // useEffect(() => {
  //     const getRaster = async () => {
  //         return await loader?.[0].getRaster({selection: {c: 0, t: 0, z: 0}})
  //     }
  //     getRaster().then(raster => {
  //         console.log('use effect', raster, loader)
  //         console.log('selections', selections)
  //     });
  //
  // }, [loader, selections])

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
    useChannelsStore.setState({
      channelsVisible: _tmpChannelsVisible,
    });
  };

  const calculateContrastLimits = () => {
    context?.setIsLoading(true);

    loader?.[pyramidResolution]
      ?.getRaster({
        selection: selections[channelIndex],
      })
      .then((raster) => {
        console.log("raster", raster);
        const conrastLimits = psudoAnalysis.channel_gmm(raster.data);
        const intContrastLimits = [
          _.toInteger(conrastLimits[0]),
          _.toInteger(conrastLimits[1]),
        ];

        console.log("contrast limits", intContrastLimits, raster.data);
        setPropertiesForChannel(ind, { contrastLimits: intContrastLimits });
      })
      .finally(() => {
        context?.setIsLoading(false);
      });

    console.log("calculate contrast limits", channelIndex);
  };

  function colorArrayToRGB(color) {
    return `rgb(${color[0]}, ${color[1]}, ${color[2]})`;
  }

  const handleChange = (color, e) => {
    setPickerColor(color.rgb);
    colors[channelIndex] = [pickerColor.r, pickerColor.g, pickerColor.b];
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
            sx={{ height: "100%" }}
            direction="row"
            justifyContent="flex-start"
            alignItems="center"
            ref={ref}
          >
            <Select
              native
              value={channelOptions[selections[ind].c]}
              onChange={onSelectionChange}
              variant="standard"
              className={"simon"}
            >
              {channelOptions.map((opt) => (
                <option key={opt} value={opt}>
                  {opt}
                </option>
              ))}
            </Select>
            <Grid item xs={1} sx={{ paddingTop: "20px" }}>
              <IconButton color="primary" onClick={toggleVisibility}>
                {channelsVisible?.[channelIndex] && <VisibilityIcon />}
                {!channelsVisible?.[channelIndex] && <VisibilityOffIcon />}
              </IconButton>
            </Grid>
            {channelsVisible?.[channelIndex] && (
              <Grid item xs={1} sx={{ paddingTop: "20px" }}>
                <IconButton color="primary" onClick={toggleLock}>
                  {colorLocked && <LockIcon />}
                  {!colorLocked && <LockOpenIcon />}
                </IconButton>
              </Grid>
            )}
            {channelsVisible?.[channelIndex] && (
              <Grid
                item
                xs={4}
                sx={{ paddingLeft: "15px", paddingTop: "20px" }}
              >
                <ColorNameSelect
                  label={"Color Name"}
                  channelIndex={channelIndex}
                  multiSelect={false}
                />
              </Grid>
            )}
            {channelsVisible?.[channelIndex] && (
              <Grid
                item
                xs={2}
                sx={{ paddingLeft: "10px", paddingTop: "25px" }}
              >
                <Button
                  aria-controls={openEl ? "basic-menu" : undefined}
                  aria-haspopup="true"
                  aria-expanded={openEl ? "true" : undefined}
                  onClick={handleElClick}
                  sx={{
                    maxWidth: "35px",
                    maxHeight: "35px",
                    minWidth: "35px",
                    minHeight: "35px",
                    border: "1px solid black",
                    background: colorArrayToRGB(colors[channelIndex]),
                    ":hover": {
                      bgcolor: colorArrayToRGB(colors[channelIndex]),
                      color: colorArrayToRGB(colors[channelIndex]),
                    },
                  }}
                ></Button>
                <Menu
                  id="basic-menu"
                  anchorEl={anchorEl}
                  open={openEl}
                  onClose={handleCloseEl}
                  MenuListProps={{
                    "aria-labelledby": "basic-button",
                  }}
                >
                  <ChromePicker
                    disableAlpha={true}
                    color={pickerColor}
                    onChange={handleChange}
                  />
                </Menu>
              </Grid>
            )}
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
                  marginTop: "7px",
                }}
              />
            </Grid>
            <Grid
              item
              xs={4}
              sx={{ paddingLeft: "15px", paddingRight: "15px" }}
            >
              <IconButton color="primary" onClick={calculateContrastLimits}>
                <ContrastIcon />
              </IconButton>
            </Grid>
          </Grid>
        </>
      )}
    </>
  );
}

export default ChannelColorDisplay;
