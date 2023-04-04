import React, {useContext, useEffect, useRef} from 'react';
import _ from 'lodash';
import * as d3 from "d3";
import {AppContext} from "../context/GlobalContext.jsx";
import {useImageSettingsStore} from "../Avivator/state.js";
import shallow from "zustand/shallow";
import GLSL from 'glsl-transpiler';

function ColorLegend(props) {

    const [fragmentShader, colormap] = useImageSettingsStore(store => [store.fragmentShader, store.colormap], shallow);

    const context = useContext(AppContext);
    const {color, referenceCarrot, pickable} = props;
    const svgHeight = referenceCarrot ? 115 : 50;
    const legendRef = useRef()


    useEffect(() => {
            if (!legendRef || !color || !fragmentShader) return;
            let svg = d3.select(legendRef.current)
            svg.selectAll("*").remove();
            let compile = GLSL();
            let result = compile(fragmentShader);
            result = result.concat('\nreturn color_transfer_srgb(param1, param2);');
            let colorTransfer = new Function('param1', 'param2', result);
            let rgbColor = context.hex2rgb(color).map(x => x / 255.0);

            let scale = d3.scaleLinear()
                .domain([0, 65535])
                .range(['#000000', color]);
            let range = _.range(0, 65535, 300);


            let xScale = d3.scaleLinear().domain([0, 65535]).range([20, _.size(range) + 20])

            let hoverRect = svg.selectAll('.hover-rect')
                .data([0])
                .join('rect')
                .attr('x', 0)
                .attr('y', svgHeight - 40)
                .attr('class', 'hover-rect')
                .attr('width', 4)
                .attr('height', 40)
                .attr('fill', 'none')

            let rects = svg.selectAll('.color-lines')
                .data(range)
                .join('rect')
                .attr('x', (d) => xScale(d))
                .attr('y', svgHeight - 30)
                .attr('class', 'color-lines')
                .attr('width', 2)
                .attr('height', 20)
                .attr('fill', d => {
                    let rgb = colorTransfer(rgbColor, d / 65535.0);
                    let intRgb = rgb.map(x => Math.min(Math.max(Math.round(x * 255), 0), 255));
                    return context.rgb2hex(intRgb);
                })
                .style('cursor', () => {
                    if (pickable) return 'pointer';
                    return 'default';
                })
                .on('mouseover', (event, d, i) => {
                    if (!pickable) return;
                    hoverRect
                        .attr('fill', 'rgba(255, 255, 255, 0.5)')
                        .attr('x', () => xScale(d))
                })
                .on('click', (event, d, i) => {
                    if (!pickable) return;
                    selectionRect
                        .attr('fill', 'rgba(255, 255, 255, 1)')
                        .attr('x', () => xScale(d))
                    context.setQuantityLegendSelection(d);

                })


            svg.append('rect')
                .attr('x', 19)
                .attr('y', svgHeight - 31)
                .attr('width', _.size(range) + 2)
                .attr('height', 22)
                .attr('fill', 'none')
                .attr('stroke', 'white')
                .attr('stroke-width', "2")
                .on('mouseout', (event, d, i) => {
                    if (!pickable) return;
                    hoverRect
                        .attr('fill', 'rgba(255, 255, 255, 0)')
                })


            let selectionRect = svg.selectAll('.selection-rect')
                .data([0])
                .join('rect')
                .attr('x', 0)
                .attr('y', svgHeight - 40)
                .attr('class', 'selection-rect')
                .attr('width', 4)
                .attr('height', 40)
                .attr('fill', 'none')


            if (referenceCarrot) {
                let sym = d3.symbol().type(d3.symbolTriangle).size(200);
                let carrotOffset = (range.findIndex(n => referenceCarrot <= n)) + 20;
                if (carrotOffset === 19 && referenceCarrot === 65535) {
                    carrotOffset = _.size(range) + 19;
                }

                svg.append("path")
                    .attr("d", sym)
                    .attr("fill", "white")
                    .attr("transform", `translate(${carrotOffset}, ${svgHeight - 45}) rotate(180)`);

                svg.append('line')
                    .attr('x1', carrotOffset)
                    .attr('y1', svgHeight - 80)
                    .attr('x2', carrotOffset)
                    .attr('y2', svgHeight - 45)
                    .attr('stroke', 'white')
                    .attr('stroke-width', 3)
                    .classed('carrot-line', true)

                svg.append('rect')
                    .attr('x', carrotOffset - 15)
                    .attr('y', svgHeight - 110)
                    .attr('width', 30)
                    .attr('height', 30)
                    .attr('fill', scale(referenceCarrot))
                    .attr('stroke', 'white')
                    .attr('stroke-width', 2)


            }


        }

        ,
        [legendRef, color, referenceCarrot, fragmentShader, colormap]
    )


    return (<>
        <svg width="258" height={svgHeight} ref={legendRef}></svg>

    </>);
}

export default ColorLegend