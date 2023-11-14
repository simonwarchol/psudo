import React, { useState } from "react";

export const AppContext = React.createContext(null);

export const COLORMAP_OPTIONS = ["sRGB", "Oklab"];

export const ContextWrapper = (props) => {
  const [isLoading, setIsLoading] = useState(false);
  const [lockedChannelColors, setLockedChannelColors] = useState([]);
  const [channelColorNames, setChannelColorNames] = useState([]);
  const [colorExcluded, setColorExcluded] = useState([]);
  const [showOptimizedColor, setShowOptimizedColor] = useState(false);
  const [pastPalettes, setPastPalettes] = useState([]);
  const [graphData, setGraphData] = useState([]);
  const [lensData, setLensData] = useState([]);
  const [optimizationScope, setOptimizationScope] = useState("global");
  const [overlapView, setOverlapView] = useState(false);
  const [linkedViews, setLinkedViews] = useState(false);
  const [mainViewStateChanged, setMainViewStateChanged] = useState(false);
  const [paletteLoss, setPaletteLoss] = useState({});

  const rgb2hex = (rgb) => {
    try {
      return (
        "#" +
        ((1 << 24) + (rgb[0] << 16) + (rgb[1] << 8) + rgb[2])
          .toString(16)
          .substr(1)
      );
    } catch (e) {
      console.log("Error in hex2rgb", rgb, e);
    }
  };

  function hex2rgb(hex) {
    try {
      let result = /^#?([a-f\d]{2})([a-f\d]{2})([a-f\d]{2})$/i.exec(hex);
      return result
        ? [
            parseInt(result[1], 16),
            parseInt(result[2], 16),
            parseInt(result[3], 16),
          ]
        : null;
    } catch (e) {
      console.log("Error in hex2rgb", hex, e);
    }
  }

  return (
    <AppContext.Provider
      value={{
        rgb2hex,
        hex2rgb,
        isLoading,
        setIsLoading,
        lockedChannelColors,
        setLockedChannelColors,
        channelColorNames,
        setChannelColorNames,
        colorExcluded,
        setColorExcluded,
        showOptimizedColor,
        setShowOptimizedColor,
        pastPalettes,
        setPastPalettes,
        optimizationScope,
        setOptimizationScope,
        graphData,
        setGraphData,
        lensData,
        setLensData,
        overlapView,
        setOverlapView,
        mainViewStateChanged,
        setMainViewStateChanged,
        linkedViews,
        setLinkedViews,
        paletteLoss,
        setPaletteLoss,
      }}
    >
      {props.children}
    </AppContext.Provider>
  );
};
