import React, {useContext, useEffect, useState} from 'react';
import {AppContext} from "../context/GlobalContext.jsx";
import {
    Button,
    FormControlLabel,
    FormLabel,
    Grid,
    InputLabel,
    Menu,
    MenuItem,
    Radio,
    RadioGroup,
    Select
} from "@mui/material";
import _ from 'lodash'
import {useChannelsStore, useImageSettingsStore, useLoader, useViewerStore} from "./state.js";
import shallow from "zustand/shallow";
import MiniAvivator from "./MiniAvivator.jsx";
import ChannelColorDisplay from "../components/ChannelColorDisplay.jsx";
import {createTheme, ThemeProvider} from "@mui/material/styles";
import {makeBoundingBox} from '@vivjs/layers';
import FormControl from "@mui/material/FormControl";
import ColorNameSelect from "../components/ColorNameSelect.jsx";
import {GLOBAL_SLIDER_DIMENSION_FIELDS} from "./constants.js";
import GlobalSelectionSlider from "./components/Controller/components/GlobalSelectionSlider.jsx";
import ChannelList from "../components/ChannelList.jsx";
import LayersIcon from '@mui/icons-material/Layers';
import * as psudoAnalysis from "psudo-analysis";
import fs from "fs";


// import lodash
function IndividualChannelsWrapper() {
    const context = useContext(AppContext);
    const darkTheme = createTheme({
        palette: {
            mode: 'dark',
        },
    });

    const colorOptions = [
        {value: 'Oklab', label: 'OKLAB'},
        {value: 'sRGB', label: 'sRGB'},
    ];
    const [globalSelection, channelOptions, source, viewState, pyramidResolution] = useViewerStore(
        store => [store.globalSelection, store.channelOptions, store.source, store.viewState, store.pyramidResolution],
        shallow
    );

    const [colorSpace, setColorSpace] = useState(colorOptions[0].value);
    useEffect(() => {
        if (!colorSpace) return;
        console.log('Changing', colorSpace);
        useImageSettingsStore.setState({colormap: colorSpace});
        //     Color Space Update
    }, [colorSpace]);
    const handleChange = (event) => {
        setColorSpace(event.target.value);
    }

    const loader = useLoader();
    const {shape, labels} = loader[0];
    const globalControlLabels = labels.filter(label => GLOBAL_SLIDER_DIMENSION_FIELDS.includes(label));
    const globalControllers = globalControlLabels.map(label => {
        const size = shape[labels.indexOf(label)];
        // Only return a slider if there is a "stack."
        return size > 1 ? (<GlobalSelectionSlider key={label} size={size} label={label}/>) : null;
    });


    const [anchorEl, setAnchorEl] = useState(null);
    const openEl = Boolean(anchorEl);
    const handleElClick = (event) => {
        setAnchorEl(event.currentTarget);
    };
    const handleCloseEl = () => {
        setAnchorEl(null);
    };


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

    const savePastPalette = async () => {
        const visibleIndices = selections.map(s => s.c).filter((d, i) => channelsVisible[i]);
        const fetchUrl = new URL(`${import.meta.env.VITE_BACKEND_URL}/save_past_palette`);
        const params = {
            pathName: context?.randomState?.dataPath,
            'channelsVisible': JSON.stringify(visibleIndices),
            colorSpace: colorSpace,
            colors,
            contrastLimits: JSON.stringify(contrastLimits),
            optimizationScope: context.optimizationScope === 'global' ? JSON.stringify({}) : JSON.stringify(makeBoundingBox(viewState)),
            z: selections[0].z


        };
        fetchUrl.search = new URLSearchParams(params).toString();
        const response = await fetch(fetchUrl);
        const newColors = response.ok ? await response.json() : {}
        console.log('New Colors', newColors)
        context.setPastPalettes([newColors, ...context.pastPalettes])

    }

    const handleChangeOptimizationScope = (event) => {
        context.setOptimizationScope(event.target.value);
    }

    const optimize = async () => {
        const channelsPayload = []
        channelsVisible.forEach((d, i) => {
            if (d) {
                const channelPayload = {
                    color: colors[i],
                    contrastLimits: contrastLimits[i],
                    selection: selections[i]
                }
                channelsPayload.push(channelPayload)
            }
        })
        const uint16Colors = new Uint16Array(channelsVisible.length * 3)
        const uint16ContrastLimits = new Uint16Array(channelsVisible.length * 2);
        const uint16Width = new Uint16Array(1);
        const uint16Height = new Uint16Array(1);
        const uint16ChannelRaster = await (Promise.all(channelsPayload.map(async (d, i) => {
            uint16Colors[i * 3] = d.color[0]
            uint16Colors[i * 3 + 1] = d.color[1]
            uint16Colors[i * 3 + 2] = d.color[2]
            uint16ContrastLimits[i * 2] = d.contrastLimits[0]
            uint16ContrastLimits[i * 2 + 1] = d.contrastLimits[1];
            const raster = await loader?.[pyramidResolution]?.getRaster({selection: d.selection})
            uint16Width[0] = raster.width;
            uint16Height[0] = raster.height;
            return raster.data
        })))
        // const test = psudoAnalysis.apply_ramp_function(uint16ChannelRaster[0], uint16ContrastLimits.slice(0, 2))
        // const test2 = psudoAnalysis.color_to_oklab(uint16Colors.slice(0, 3))
        // Declare f32 array with 1.0 and 2.0

        // let float32Colors = new Float32Array(uint16Colors.length);

        const test = psudoAnalysis.color_test(new Float32Array([0.5, 0.2]));
        console.log('test2', test)
        // console.log('test', JSON.stringify({'arr': Array.from(test)}))
        // console.log('Adding one', psudoAnalysis.add_one(444))
        // console.log('uint16Colors', uint16Colors, uint16ContrastLimits, uint16ChannelRaster)
    }


    const getNewPalette = async () => {
        context?.setIsLoading(true)
        await savePastPalette()
        const visibleIndices = selections.map(s => s.c).filter((d, i) => channelsVisible[i]);
        const fetchUrl = new URL(`${import.meta.env.VITE_BACKEND_URL}/get_auto_palette`);
        const params = {
            dataPath: source?.urlOrFile,
            colors: colors,
            'lockedChannelColors': context.lockedChannelColors,
            'channelColorNames': JSON.stringify(context.channelColorNames),
            'colorexcluded': context.colorExcluded,
            'colorSpace': colorSpace,
            contrastLimits: JSON.stringify(contrastLimits),
            'channelsVisible': JSON.stringify(visibleIndices),
            globalSelection: JSON.stringify(globalSelection),
            optimizationScope: context.optimizationScope === 'global' ? JSON.stringify({}) : JSON.stringify(makeBoundingBox(viewState)),
            z: selections[0].z
        };
        fetchUrl.search = new URLSearchParams(params).toString();
        const response = await fetch(fetchUrl);
        const newColors = response.ok ? await response.json() : [];
        let ctr = 0
        const _tmpColors = _.cloneDeep(colors).map((color, i) => {
            if (channelsVisible[i]) {
                return newColors['newPalette'][ctr++]
            } else {
                return color
            }
        });
        context?.setIsLoading(false)

        context?.setShowOptimizedColor(true);
        if (_tmpColors) useChannelsStore.setState({colors: _tmpColors, prevColors: colors});
    }


    return (
        <>
            <ThemeProvider theme={darkTheme}>
                <Grid container
                      direction="row" justifyContent="center"
                      alignItems="center" item xs={'auto'}>
                    <Grid item xs={12}>
                        <Button endIcon={<LayersIcon/>}
                                id="channels-button"
                                aria-controls={openEl ? 'basic-menu' : undefined}
                                aria-haspopup="true"
                                aria-expanded={openEl ? 'true' : undefined}
                                onClick={handleElClick}

                        >
                            Channels
                        </Button>
                        <Menu
                            id="basic-menu"
                            anchorEl={anchorEl}
                            open={openEl}
                            onClose={handleCloseEl}
                            MenuListProps={{
                                'aria-labelledby': 'basic-button',
                            }}
                        >
                            <ChannelList/>
                        </Menu>
                    </Grid>
                    <Grid item xs={6}>
                        {globalControllers}
                    </Grid>
                </Grid>


                {channelsVisible.map((d, i) => (
                    <>
                        {
                            d == true && (
                                <Grid container item xs={true}
                                      key={`mini__avivator_${i}_${JSON.stringify(i)}`}>
                                    <Grid item xs={6}>
                                        <MiniAvivator channelsVisible={d} allowNavigation={false}
                                                      showChannel={i}/>
                                    </Grid>
                                    <Grid item xs={6}>
                                        <ChannelColorDisplay channelIndex={i}/>
                                    </Grid>

                                </Grid>
                            )
                        }
                    </>
                ))}

                <>
                    <Grid container direction="row" justifyContent="center" alignItems="center"
                          sx={{
                              margin: 2, paddingRight: 10, paddingLeft: 10
                          }}>
                        <Grid item xs={6} p={1}>
                            <ColorNameSelect label={"Excluded Colors"} multiSelect={true}/>
                        </Grid>
                        <Grid item xs={6} p={1}>
                            <FormControl>
                                <FormLabel id="optimization-scope-grouplabel">Optimization Scope</FormLabel>
                                <RadioGroup
                                    row
                                    aria-labelledby="optimization-scope-grouplabel"
                                    value={context.optimizationScope}
                                    onChange={handleChangeOptimizationScope}
                                >
                                    <FormControlLabel value="global" control={<Radio/>} label="Global"/>
                                    <FormControlLabel value="viewport" control={<Radio/>} label="Viewport"/>
                                </RadioGroup>
                            </FormControl>
                        </Grid>
                        <Grid item xs={6} p={1}>
                            <FormControl className={'color-space-form'}>
                                <InputLabel id="color-space-select-label">Color
                                    Space</InputLabel>
                                <Select
                                    id="color-space"
                                    value={colorSpace}
                                    defaultValue={colorOptions[0].value}
                                    onChange={handleChange}
                                    size="small"
                                >
                                    {colorOptions.map((option) => (
                                        <MenuItem key={option.value} value={option.value}>
                                            {option.label}
                                        </MenuItem>
                                    ))}
                                </Select>
                            </FormControl>
                        </Grid>
                        <Grid item xs={6} p={1}>
                            <Button variant="contained" onClick={optimize}
                            >Optimize Palette</Button>
                        </Grid>
                    </Grid>
                </>
            </ThemeProvider>
        </>
    )
        ;
}

export default IndividualChannelsWrapper