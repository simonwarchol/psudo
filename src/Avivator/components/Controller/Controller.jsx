import React from "react";
import { CircularProgress, Divider, Grid } from "@mui/material";

import shallow from "zustand/shallow";

import ChannelController from "./components/ChannelController";
import Menu from "./components/Menu";
import ColormapSelect from "./components/ColormapSelect";
import GlobalSelectionSlider from "./components/GlobalSelectionSlider";
import AddChannel from "./components/AddChannel";
import {
  useChannelsStore,
  useImageSettingsStore,
  useLoader,
  useMetadata,
  useViewerStore,
} from "../../state";
import { getSingleSelectionStats, guessRgb } from "../../viewerUtils";
import { GLOBAL_SLIDER_DIMENSION_FIELDS } from "../../constants";

function TabPanel(props) {
  const { children, value, index, ...other } = props;

  return (
    <div
      role="tabpanel"
      hidden={value !== index}
      id={`simple-tabpanel-${index}`}
      aria-labelledby={`simple-tab-${index}`}
      // eslint-disable-next-line react/jsx-props-no-spreading
      {...other}
    >
      {value === index && children}
    </div>
  );
}

const Controller = () => {
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
  const loader = useLoader();

  const colormap = useImageSettingsStore((store) => store.colormap);
  const [
    channelOptions,
    useColormap,
    isChannelLoading,
    setIsChannelLoading,
    removeIsChannelLoading,
    pixelValues,
    isViewerLoading,
    pyramidResolution
  ] = useViewerStore(
    (store) => [
      store.channelOptions,
      store.useColormap,
      store.isChannelLoading,
      store.setIsChannelLoading,
      store.removeIsChannelLoading,
      store.pixelValues,
      store.isViewerLoading,
        store.pyramidResolution
    ],
    shallow
  );
  const metadata = useMetadata();
  const isRgb = metadata && guessRgb(metadata);
  const { shape, labels } = loader[0];
  const globalControlLabels = labels.filter((label) =>
    GLOBAL_SLIDER_DIMENSION_FIELDS.includes(label)
  );
  const channelControllers = ids.map((id, i) => {
    const onSelectionChange = (e) => {
      console.log("selection change");
      const selection = {
        ...selections[i],
        c: channelOptions.indexOf(e.target.value),
      };
      setIsChannelLoading(i, true);
      getSingleSelectionStats({
        loader,
        selection,
        pyramidResolution
      }).then(({ domain, contrastLimits: newContrastLimit }) => {
        console.log("domain", domain, "contrastLimits", newContrastLimit);
        setPropertiesForChannel(i, {
          contrastLimits: newContrastLimit,
          domains: domain,
        });
        // useImageSettingsStore.setState({
        //     onViewportLoad: () => {
        //         useImageSettingsStore.setState({
        //             onViewportLoad: () => {
        //             }
        //         });
        //
        //     }
        // });
        setIsChannelLoading(i, false);
        setPropertiesForChannel(i, { selections: selection });
      });
    };
    const toggleIsOn = () => toggleIsOnSetter(i);
    const handleSliderChange = (e, v) =>
      setPropertiesForChannel(i, { contrastLimits: v });
    const handleRemoveChannel = () => {
      removeChannel(i);
      removeIsChannelLoading(i);
    };
    const handleColorSelect = (color) => {
      setPropertiesForChannel(i, { colors: color });
    };
    const name = channelOptions[selections[i].c];
    return (
      <Grid
        key={`channel-controller-${name}-${id}`}
        style={{ width: "100%" }}
        item
      >
        <ChannelController
          name={name}
          onSelectionChange={onSelectionChange}
          channelsVisible={channelsVisible[i]}
          pixelValue={pixelValues[i]}
          toggleIsOn={toggleIsOn}
          handleSliderChange={handleSliderChange}
          domain={domains[i]}
          slider={contrastLimits[i]}
          color={colors[i]}
          handleRemoveChannel={handleRemoveChannel}
          handleColorSelect={handleColorSelect}
          isLoading={isChannelLoading[i]}
        />
      </Grid>
    );
  });
  const globalControllers = globalControlLabels.map((label) => {
    const size = shape[labels.indexOf(label)];
    // Only return a slider if there is a "stack."
    return size > 1 ? (
      <GlobalSelectionSlider key={label} size={size} label={label} />
    ) : null;
  });

  return (
    <Menu>
      <TabPanel value={0} index={0}>
        {useColormap && <ColormapSelect />}
        {globalControllers}
        {!isViewerLoading && !isRgb ? (
          <Grid container>{channelControllers}</Grid>
        ) : (
          <Grid container justifyContent="center">
            {!isRgb && <CircularProgress />}
          </Grid>
        )}
        {!isRgb && <AddChannel />}
      </TabPanel>
      <Divider
        style={{
          marginTop: "8px",
          marginBottom: "8px",
        }}
      />
    </Menu>
  );
};
export default Controller;
