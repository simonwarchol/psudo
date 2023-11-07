import React, { useContext, useEffect, useState } from "react";
import { AppContext } from "../context/GlobalContext.jsx";
import { VegaLite } from "react-vega";

const LineChart = () => {
  const context = useContext(AppContext);
  const [lineChartData, setLineChartData] = useState([]);

  useEffect(() => {
    const dat = [];
    (context.graphData || []).forEach((d, index) => {
      (d?.data?.frequencies || []).forEach((f) => {
        dat.push({
          x: f[0],
          y: f[1],
          color: `rgb(${d.color.join(",")})`,
          line: `Line ${index}`, // Adding a line field to differentiate lines
        });
      });
    });
    console.log("dat", dat);
    setLineChartData(dat);
  }, [context.graphData]);

  // Specify the Vega-Lite spec for a multi-line chart
  const spec = {
    data: { values: lineChartData }, // use the data state
    mark: "line", // specify the mark type as line
    encoding: {
      x: {
        field: "x",
        type: "quantitative",
        axis: {
          labels: false,
          title: null,
          // labelExpr: "format( datum.value, '.0f')",
          // // labelExpr: "format(pow(2.718281828459045, datum.value), '.0f')",
          // title: "Marker Expression",
          grid: false, // this removes the x-axis grid lines
        },
      },
      y: {
        field: "y",
        type: "quantitative",
        axis: {
          labels: false,
          title: null,
          grid: false, // this removes the y-axis grid lines
        },
      },
      color: {
        field: "color",
        type: "nominal",
        scale: null,
        legend: null, // color is directly provided in the data
      },
      detail: {
        field: "line",
        type: "nominal", // use the line field to differentiate lines
      },
    },
    config: {
      axis: {
        domain: false,
        ticks: false, // this hides the axes' domain and ticks
      },
      view: {
        stroke: null, // this removes the border/stroke around the view
        padding: 0, // this sets the view padding to 0
      },
    },
    background: "#2e2e2e",
    width: 350,
    height: 60,
  };

  return <VegaLite spec={spec} actions={false} />;
};

export default LineChart;
