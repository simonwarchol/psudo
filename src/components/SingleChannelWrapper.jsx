import React, {useContext, useEffect, useState} from 'react'
import {useChannelsStore, useImageSettingsStore} from "../Avivator/state.js";
import shallow from "zustand/shallow";
import {AppContext} from "../context/GlobalContext.jsx";
import PngCanvas from "./PngCanvas.jsx";

const SingleChannelWrapper = props => {
    const context = useContext(AppContext);
    const {width, height, absoluteScale} = props;

    const channelsVisible = props?.channelsVisible;
    console.log('cv', channelsVisible)

    const {channelIndex, label} = props;
    const store = useChannelsStore(store => store, shallow);
    const [singleChannelData, setSingleChannelData] = useState(null);
    const ref = React.useRef(null);
    const [colormap] = useImageSettingsStore(store => [store.colormap], shallow);

    const [colors] =
        useChannelsStore(
            store => [
                store.colors],
            shallow
        );


    useEffect(() => {
        if (channelIndex == null) return;
        const fetchSingleChannelData = async () => {
            const fetchUrl = new URL(`${import.meta.env.VITE_BACKEND_URL}/fetch_single_channel_data`);
            const params = {
                pathName: context?.randomState?.dataPath,
                channel: channelIndex,
                colorSpace: colormap,
                color: colors[channelIndex],
                channelsVisible
            };
            fetchUrl.search = new URLSearchParams(params).toString();
            const response = await fetch(fetchUrl);
            const data = await response.json()
            if (data?.['image_data']) setSingleChannelData(data['image_data'])
        }
        fetchSingleChannelData().then(r => {
            console.log('r', r);
        })

    }, [channelIndex, colors, colormap, channelsVisible])


    return (
        <div style={{width: '128px', height: '128px', margin: '20px'}}>
            <PngCanvas label={label} data={singleChannelData} width={width || 512} height={height || 512}
                       absoluteScale={absoluteScale || 128 / 512}/>
        </div>
    )
}

export default SingleChannelWrapper