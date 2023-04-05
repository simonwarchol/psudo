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

function DeleteIcon() {
    return null;
}


function LayersIcon() {
    return null;
}

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
                            <Button variant="contained" onClick={getNewPalette}
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