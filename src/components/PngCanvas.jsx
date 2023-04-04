import React, {useContext, useEffect, useRef} from 'react'
import {useChannelsStore} from "../Avivator/state.js";
import shallow from "zustand/shallow";
import {AppContext} from "../context/GlobalContext.jsx";

const PngCanvas = props => {
    const context = useContext(AppContext);

    const {width, height, data} = props;
    const scapeFactor = props.scaleFactor || 1;
    const absoluteScale = props?.absoluteScale;
    const store = useChannelsStore(store => store, shallow);


    const canvasRef = useRef(null)

    useEffect(() => {
        if (!data) return;

        const canvas = canvasRef.current
        let ctx = canvas.getContext("2d");
        let imgData = ctx.createImageData(width, height);
        imgData.data.set(data);
        ctx.putImageData(imgData, 0, 0);
        // ctx.scale(100, 100)
        //Our draw come here
    }, [data])
    // const style = {'transform': `scale(${scale},10)`, height, width}

    useEffect(() => {
        if (!canvasRef) return;
        // canvasRef.current.style.padding = '100px';
        let transform;
        if (absoluteScale) {
            transform = `scale(${absoluteScale},${absoluteScale})`
        } else {
            transform = `scale(${context.scale[0] * scapeFactor},${context.scale[1] * scapeFactor})`
        }
        console.log(transform, 'tr')
        canvasRef.current.style.transform = transform;
        canvasRef.current.style.border = '1px solid white';


    }, [canvasRef, context.scale])


    return (
        <canvas ref={canvasRef} {...props}  />
    )
}

export default PngCanvas