import React, { useContext, useEffect, useState } from "react";
import { AppContext } from "../context/GlobalContext.jsx";
import GaugeChart from "react-gauge-chart";
import Grid from "@mui/material/Grid";
import ThumbUpIcon from "@mui/icons-material/ThumbUp";
import ThumbDownIcon from "@mui/icons-material/ThumbDown";
import _ from "lodash";

const GaugeCharts = () => {
  const context = useContext(AppContext);
  const [nameDistance, setNameDistance] = useState(0);
  const [perceptualDistance, setPerceptualDistance] = useState(0);
  const [confusion, setConfusion] = useState(0);
  const [overallLoss, setOverallLoss] = useState(0);
  const chartStyle = {
    zoom: 0.4,
  };
  // let chartColors = ["#fee0d2", "#fc9272", "#de2d26"];
  let chartColors = ["#f0f0f0", "#bdbdbd", "#636363"];

  useEffect(() => {
    console.log("Gauge Chart Data Simon", context.paletteLoss);
    if (context.paletteLoss) {
      let nameDist = 1 - (context?.paletteLoss?.name_distance || 0) * -1;
      let perceptDist =
        1 - (context?.paletteLoss?.perceptural_distance || 0) * -1;
      let conf = 1 - (context?.paletteLoss?.confusion || 0) * -1;
      setNameDistance(nameDist);
      setPerceptualDistance(perceptDist);
      setConfusion(conf);
      setOverallLoss(nameDist + perceptDist + conf);
    }
  }, [context.paletteLoss]);

  const iconStyle = {
    fontSize: "16px", // Set the font size for icons
  };

  return (
    <Grid container direction="column" spacing={2}>
      {/* Perceptual Distance Section */}
      <Grid item>
        <Grid container alignItems="center" justifyContent="center">
          <Grid item>
            <GaugeChart
              id="gauge-chart-perceptual"
              colors={chartColors}
              percent={perceptualDistance}
              animate={false}
              style={chartStyle}
              hideText={true}
            />
          </Grid>
        </Grid>
        <Grid container justifyContent="center">
          <Grid item>
            <label>Perceptual Similarity</label>
          </Grid>
        </Grid>
      </Grid>

      {/* Name Distance Section */}
      <Grid item>
        <Grid container alignItems="center" justifyContent="center">
          <Grid item>
            <GaugeChart
              id="gauge-chart-name"
              percent={nameDistance}
              colors={chartColors}
              style={chartStyle}
              animate={false}
              hideText={true}
            />
          </Grid>
        </Grid>
        <Grid container justifyContent="center">
          <Grid item>
            <label>Name Similarity</label>
          </Grid>
        </Grid>
      </Grid>
      {/* Confusion Section */}
      <Grid item>
        <Grid container alignItems="center" justifyContent="center">
          <Grid item>
            <GaugeChart
              id="gauge-chart-confusion"
              percent={confusion}
              colors={chartColors}
              style={chartStyle}
              animate={false}
              hideText={true}
            />
          </Grid>
        </Grid>
        <Grid container justifyContent="center">
          <Grid item>
            <label>Confusion</label>
          </Grid>
        </Grid>
      </Grid>
      {/* Confusion Section */}
      <Grid item>
        <Grid container alignItems="center" justifyContent="center">
          <Grid item>
            <GaugeChart
              id="gauge-chart-total"
              percent={confusion}
              colors={chartColors}
              style={chartStyle}
              animate={false}
              hideText={true}
            />
          </Grid>
        </Grid>
        <Grid container justifyContent="center">
          <Grid item>
            <label>Total</label>
          </Grid>
        </Grid>
      </Grid>
    </Grid>
  );
};

export default GaugeCharts;
