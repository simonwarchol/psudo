import React, { useCallback, useEffect, useState } from "react";
// eslint-disable-next-line camelcase
import { unstable_batchedUpdates } from "react-dom";
import { Grid, Slider } from "@mui/material";
import debounce from "lodash/debounce";
import shallow from "zustand/shallow";
import {
  getMultiSelectionStats,
  range,
  truncateDecimalNumber,
} from "../../../viewerUtils";
import {
  useChannelsStore,
  useImageSettingsStore,
  useLoader,
  useViewerStore,
} from "../../../state";

export default function GlobalSelectionSlider(props) {
  const { size, label } = props;
  const [selections, setPropertiesForChannel] = useChannelsStore(
    (store) => [store.selections, store.setPropertiesForChannel],
    shallow
  );
  const loader = useLoader();
  const [globalSelection, source, pyramidResolution] = useViewerStore(
    (store) => [store.globalSelection, store.source, source.pyramidResolution]
  );
  const [scale, setScale] = useState({});

  const changeSelection = useCallback(
    debounce(
      (event, newValue) => {
        useViewerStore.setState({
          isChannelLoading: selections.map(() => true),
        });
        const newSelections = [...selections].map((sel) => ({
          ...sel,
          [label]: newValue,
        }));
        getMultiSelectionStats({
          loader,
          selections: newSelections,
          pyramidResolution,
        }).then(({ domains, contrastLimits }) => {
          unstable_batchedUpdates(() => {
            range(newSelections.length).forEach((channel, j) =>
              setPropertiesForChannel(channel, {
                domains: domains[j],
                contrastLimits: contrastLimits[j],
              })
            );
          });
          unstable_batchedUpdates(() => {
            useImageSettingsStore.setState({
              onViewportLoad: () => {
                useImageSettingsStore.setState({
                  onViewportLoad: () => {},
                });
                useViewerStore.setState({
                  isChannelLoading: selections.map(() => false),
                });
              },
            });
            range(newSelections.length).forEach((channel, j) =>
              setPropertiesForChannel(channel, {
                selections: newSelections[j],
              })
            );
          });
        });
      },
      50,
      { trailing: true }
    ),
    [loader, selections]
  );

  const displayValueFunction = (v) => {
    if (scale && scale?.shift && scale?.scale) {
      v = v * scale?.spectral_scale + scale?.spectral_scale;
    }
    return truncateDecimalNumber(v, 5);
  };
  return (
    <Grid
      container
      direction="row"
      justifyContent="space-between"
      alignItems="center"
    >
      <Grid item xs={1}>
        {"v"}:
      </Grid>
      <Grid item xs={11}>
        <Slider
          value={globalSelection[label]}
          onChange={(event, newValue) => {
            useViewerStore.setState({
              globalSelection: {
                ...globalSelection,
                [label]: newValue,
              },
            });
            if (event.type === "keydown") {
              changeSelection(event, newValue);
            }
          }}
          valueLabelFormat={(v) => displayValueFunction(v)}
          onChangeCommitted={changeSelection}
          valueLabelDisplay="auto"
          getAriaLabel={() => `${label} slider`}
          marks={range(size).map((val) => ({ value: val }))}
          min={0}
          max={size}
          orientation="horizontal"
          style={{ marginTop: "7px" }}
          step={null}
        />
      </Grid>
    </Grid>
  );
}
