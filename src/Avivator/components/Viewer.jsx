/* eslint-disable no-nested-ternary */
import shallow from "zustand/shallow";
import React, { useContext, useEffect, useState, useRef } from "react";
import {
  useChannelsStore,
  useImageSettingsStore,
  useLoader,
  useViewerStore,
} from "../state";
import { getLensIntensityValues, getGraphData } from "../viewerUtils.js";
import _ from "lodash";
import {
  DetailView,
  getDefaultInitialViewState,
  DETAIL_VIEW_ID,
} from "@vivjs/views";
import { VivViewer } from "@vivjs/viewers";

import { DEFAULT_OVERVIEW } from "../constants";
import { AppContext } from "../../context/GlobalContext";
import { ColorMixingExtension } from "../../components/shaders/index.js";
import {
  MinervaVivLensing,
  MinervaVivLensingDetailView,
} from "../MinervaVivLensing.jsx";

const Viewer = (props) => {
  const deckRef = useRef(null);
  const context = useContext(AppContext);
  const showChannel = props?.showChannel;
  const mainViewer = props?.mainViewer;
  const [mousePosition, setMousePosition] = useState([null, null]);
  const [lensRadius, setLensRadius] = useState(100);
  const [movingLens, setMovingLens] = useState(false);
  const [lensOpacity, setLensOpacity] = useState(1);
  const [coordinate, setCoordinate] = useState([]);

  const viewerRef = React.useRef(null);

  const [overlayLayers, setOverlayLayers] = useState([]);

  const [pyramidResolution, viewState] = useViewerStore(
    (store) => [store.pyramidResolution, store.viewState],
    shallow
  );

  let [colors, contrastLimits, channelsVisible, selections, pixelValues] =
    useChannelsStore(
      (store) => [
        store.colors,
        store.contrastLimits,
        store.channelsVisible,
        store.selections,
        store.pixelValues,
      ],
      shallow
    );
  if (showChannel || showChannel === 0) {
    channelsVisible = channelsVisible.map(() => false);
    channelsVisible[showChannel] = true;
  }
  // console.log('colors', colors, 'contrastLimits', contrastLimits, 'channelsVisible', channelsVisible, 'selections', selections)
  const { width, height } = props.dimensions;
  const allowNavigation = props?.allowNavigation || false;
  const loader = useLoader();
  const [
    lensSelection,
    colormap,
    resolution,
    lensEnabled,
    zoomLock,
    panLock,
    isOverviewOn,
    onViewportLoad,
    useFixedAxis,
  ] = useImageSettingsStore(
    (store) => [
      store.lensSelection,
      store.colormap,
      store.resolution,
      store.lensEnabled,
      store.zoomLock,
      store.panLock,
      store.isOverviewOn,
      store.onViewportLoad,
      store.useFixedAxis,
    ],
    shallow
  );
  let userData = {};

  const svgSize = 512;

  function svgToDataURL(svg) {
    return `data:image/svg+xml;charset=utf-8,${encodeURIComponent(svg)}`;
  }

  const getColorSpace = (colormap) => {
    return new ColorMixingExtension({ colormap });
  };

  const onClick = () => {
    // const _coordinate = useViewerStore
    //   .getState()
    //   ?.coordinate?.map((x) => Math.round(x));
  };

  useEffect(() => {
    const viewState = getDefaultInitialViewState(
      loader,
      { height, width },
      0.5
    );
    useViewerStore.setState({
      viewState: viewState,
      pyramidResolution: Math.min(
        Math.max(Math.round(-viewState?.zoom), 0),
        loader.length - 1
      ),
    });
  }, [loader, height, width]);

  const baseViewState = React.useMemo(() => {
    return getDefaultInitialViewState(loader, { height, width }, 0.5);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [loader]);

  // // If showChannel is false, do not include
  // extensions: [getColorSpace(colormap)],

  let deckProps = {
    layers: [],
  };
  let hoverHooks = {};
  let onHover = () => {};
  let onViewStateChange = () => {};
  let layerConfig = {
    loader,
    contrastLimits,
    colors,
    channelsVisible,
    selections,
  };
  let detailView = null;

  if (!showChannel && showChannel !== 0) {
    detailView = new MinervaVivLensingDetailView({
      id: DETAIL_VIEW_ID,
      height,
      width,
      mousePosition,
      lensRadius,
      lensOpacity,
    });
    layerConfig = {
      ...layerConfig,
      onViewportLoad: () => {
        if (mousePosition[0] === null || mousePosition[1] === null) {
          setMousePosition([
            Math.round((deckRef?.current?.deck?.width || 0) / 2),
            Math.round((deckRef?.current?.deck?.height || 0) / 2),
          ]);
        }
      },
      lensEnabled: lensEnabled,
      lensSelection,
      lensRadius: 100,
      extensions: [new MinervaVivLensing(colormap)],
    };
    userData = {
      mousePosition,
      setMousePosition,
      movingLens,
      setMovingLens,
      lensRadius,
      setLensRadius,
      lensOpacity,
      setLensOpacity,
      pyramidResolution,
      channelsVisible,
      selections,
      setIsLoading: context?.setIsLoading,
      graphData: context?.graphData,
      setGraphData: context?.setGraphData,
      coordinate,
      setCoordinate,
      overlapView: context?.overlapView,
      setOverlapView: context?.setOverlapView,
    };
    deckProps = {
      ...deckProps,
      ref: deckRef,
      userData: userData,
    };
    hoverHooks = {
      handleValue: (v) => useViewerStore.setState({ pixelValues: v }),
      handleCoordinate: (v) => useViewerStore.setState({ coordinate: v }),
    };
    onViewStateChange = ({ oldViewState, viewState: newViewState, viewId }) => {
      if (mainViewer) {
        console.log("VSCH")
        // console.log("onViewStateChange", oldViewState, newViewState, viewId);
        useViewerStore.setState({
          viewState: { ...newViewState, id: viewId },
          pyramidResolution: Math.min(
            Math.max(Math.round(-newViewState?.zoom), 0),
            loader.length - 1
          ),
        });
      }

      return movingLens ? oldViewState : newViewState;
    };
    onHover = (v, d, e) => {
      // console.log("Hover", v, d, e, loader);

      useViewerStore.setState({ coordinate: v?.coordinate });
    };
  } else {
    layerConfig = {
      ...layerConfig,
      lensEnabled: false,
      extensions: [getColorSpace(colormap)],
    };
    detailView = new DetailView({
      id: DETAIL_VIEW_ID,
      height,
      width,
    });
  }
  const views = [detailView];
  const layerProps = [layerConfig];
  const viewStates = [{ ...baseViewState, id: DETAIL_VIEW_ID }];
  const getLensGraphData = async () => {
    let lensData = await getLensIntensityValues(
      coordinate,
      useViewerStore.getState()?.viewState,
      loader,
      pyramidResolution,
      lensRadius,
      channelsVisible,
      selections,
      setMovingLens
    );
    let graphData = getGraphData(
      lensData,
      colors,
      lensSelection,
      channelsVisible,
      selections
    );
    context?.setGraphData(graphData);
    console.log("lensData", lensData);
    // coordinate, viewState, loader, userData;
  };

  useEffect(() => {
    console.log("lensEnabled", lensEnabled);

    if (lensEnabled) {
      getLensGraphData();
    }
  }, [lensEnabled]);

  return (
    <div className={"viewerWrapper"} onClick={onClick}>
      <VivViewer
        layerProps={layerProps}
        views={views}
        viewStates={viewStates}
        onViewStateChange={onViewStateChange}
        deckProps={deckProps}
        onHover={onHover}
      />
    </div>
  );
};
export default Viewer;

// Estimate
