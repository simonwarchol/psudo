import React, {useContext, useEffect} from 'react'
import {AppContext} from "../context/GlobalContext.jsx";
import {Grid, MenuItem} from "@mui/material";
import {createTheme, ThemeProvider} from "@mui/material/styles";
import PngCanvas from "./PngCanvas.jsx";
import {useChannelsStore, useImageSettingsStore, useViewerStore} from "../Avivator/state.js";
import shallow from "zustand/shallow";
import {makeBoundingBox} from "@vivjs/layers";

const PastPalettes = props => {
    const context = useContext(AppContext);
    const darkTheme = createTheme({palette: {mode: 'dark'}});

    let [channelsVisible, colors, contrastLimits, selections] =
        useChannelsStore(
            store => [
                store.channelsVisible,
                store.colors,
                store.contrastLimits,
                store.selections,

            ],
            shallow
        );
    const colormap = useImageSettingsStore(store => store.colormap);

    const [viewState] = useViewerStore(store => [store.viewState], shallow);


    useEffect(() => {
        console.log('pp, ', context?.pastPalettes)
    }, [context?.pastPalettes])

    const selectOldPalette = async (palette) => {
        console.log('clickclick', palette)
        if (context.pastPalettes?.[0] && !context.pastPalettes?.[0]?.fromHistoryClick) {
            await savePastPalette()
        }
        const newdata = []
        for (let i = 0; i < palette.colors.length; i += 3) { // i+=3 can solve your problem
            const three = [palette.colors[i], palette.colors[i + 1], palette.colors[i + 2]]
            newdata.push(three)
        }
        // transform colors into list of [rgb]

        // const oldPaletteColors = palette?.['channel_indices']?.map(d => {
        //     const channelColor = [palette['colors'][d * 3], palette['colors'][d * 3 + 1], palette['colors'][d * 3 + 2]]
        //     return channelColor;
        // })
        console.log('Old Palette Colors', newdata)
        useChannelsStore.setState({colors: newdata})
    }
    const savePastPalette = async () => {
        const visibleIndices = selections.map(s => s.c).filter((d, i) => channelsVisible[i]);

        const fetchUrl = new URL(`${import.meta.env.VITE_BACKEND_URL}/save_past_palette`);
        const params = {
            pathName: context?.randomState?.dataPath,
            'channelsVisible': JSON.stringify(visibleIndices),
            colorSpace: colormap,
            colors,
            contrastLimits: JSON.stringify(contrastLimits),
            optimizationScope: context.optimizationScope === 'global' ? JSON.stringify({}) : JSON.stringify(makeBoundingBox(viewState)),
            z: selections[0].z


        };
        fetchUrl.search = new URLSearchParams(params).toString();
        const response = await fetch(fetchUrl);
        const newColors = response.ok ? await response.json() : {}
        newColors['fromHistoryClick'] = true
        context.setPastPalettes([newColors, ...context.pastPalettes])

    }


    return (<ThemeProvider theme={darkTheme}>
        <div style={{overflowY: 'hidden'}}>

            <Grid container justifyContent="flex-start" direction="column" sx={{height: '100%', width: '100%'}}>
                <Grid item m={1} sx={{borderBottom: '1px solid white'}}>
                    <h4 style={{margin: 0}}>Palette History</h4>
                </Grid>
                {context?.pastPalettes?.map((palette, index) => {
                    return (
                        <MenuItem onClick={() => {
                            selectOldPalette(palette)
                        }}>
                            <Grid item container direction="row">
                                <Grid item xs={8}>
                                    <PngCanvas width={128} height={128} data={palette['image_data']} absoluteScale={1}/>
                                </Grid>
                                <Grid item xs={4} container direction="column" justifyContent="flex-start"
                                      paddingLeft={2}
                                      alignItems="center">
                                    {palette['channel_indices'].map((index) => {
                                        console.log(`rgb(${palette['colors'][index * 3]},${palette['colors'][index * 3] + 1},${palette['colors'][index * 3] + 2}`)
                                        return (<Grid item>
                                            {/*colored rectangle with rounded edges*/}
                                            <div style={{
                                                margin: '2px',
                                                width: '17px',
                                                height: '17px',
                                                backgroundColor: `rgb(${palette['colors'][index * 3]},${palette['colors'][index * 3 + 1]},${palette['colors'][index * 3 + 2]})`,
                                                borderRadius: '5px'
                                            }}/>
                                        </Grid>)
                                    })}
                                </Grid>
                            </Grid>
                        </MenuItem>)
                })}
            </Grid>

        </div>
    </ThemeProvider>)
}

export default PastPalettes;