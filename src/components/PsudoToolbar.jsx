import React, {useContext, useEffect, useState} from 'react';
import {AppContext} from "../context/GlobalContext.jsx";
import AddPhotoAlternateIcon from '@mui/icons-material/AddPhotoAlternate';
import {Grid, IconButton, Menu, TextField, Typography} from "@mui/material";
import {createTheme, ThemeProvider} from "@mui/material/styles";
import {useChannelsStore, useViewerStore} from "../Avivator/state.js";
import {getNameFromUrl} from "../Avivator/viewerUtils.js";
import HistoryIcon from '@mui/icons-material/History';
import PastPalettes from "./PastPalettes.jsx";
import JoinInnerIcon from '@mui/icons-material/JoinInner';
import shallow from "zustand/shallow";
import SingleChannelWrapper from "./SingleChannelWrapper.jsx";
import {AttachFile, InsertLink} from "@mui/icons-material";
import DropzoneButton from "../Avivator/components/Controller/components/DropzoneButton.jsx";
import Footer from "../Avivator/components/Footer.jsx";

function PsudoToolbar() {
    const context = useContext(AppContext);
    const [anchorElSource, setAnchorElSource] = useState(null);
    const openSource = Boolean(anchorElSource);
    const [anchorElHistory, setAnchorElHistory] = useState(null);
    const openHistory = Boolean(anchorElHistory);
    const [anchorElOverlap, setAnchorElOverlap] = useState(null);
    const openOverlap = Boolean(anchorElOverlap);
    const [showFileUpload, setShowFileUpload] = useState(false);
    const [showLinkInput, setShowLinkInput] = useState(false);
    const [source] = useViewerStore(store => [store.source], shallow);
    const [imageUrl, setImageUrl] = useState(source?.urlOrFile);
    let [channelsVisible] = useChannelsStore(store => [store.channelsVisible], shallow);
    useEffect(() => {
        setImageUrl(source?.urlOrFile)
    }, [source])


    const handleFileButtonClick = () => {
        setShowFileUpload(true);
        setShowLinkInput(false);
    };

    const handleLinkButtonClick = () => {
        setShowFileUpload(false);
        setShowLinkInput(true);
    };


    const handleSourceClick = (event) => {
        console.log(source);
        setAnchorElSource(event.currentTarget);
    };

    const handleCloseSource = () => {
        setAnchorElSource(null);
    };
    const handleOverlapClick = (event) => {
        console.log(source);
        setAnchorElOverlap(event.currentTarget);
    };

    const handleCloseOverlap = () => {
        setAnchorElOverlap(null);
    };


    const handleHistoryClick = (event) => {
        setAnchorElHistory(event.currentTarget);
    };

    const handleCloseHistory = () => {
        setAnchorElHistory(null);
    };
    const darkTheme = createTheme({palette: {mode: 'dark'}});

    const updateUrl = (e) => {
        console.log(e, 'e')
        setImageUrl(e.target.value)
    }

    const submitUrl = (e) => {
        if (e.key === 'Enter') {
            updateSourceUrl()
        }
    }

    const updateSourceUrl = () => {
        useViewerStore.setState({
            source: {
                urlOrFile: imageUrl, description: getNameFromUrl(imageUrl)
            }
        });
    }


    return (<ThemeProvider theme={darkTheme}>
        <Grid container sx={{top: 0, left: 0, position: 'absolute'}} direction="row"
              justifyContent="flex-start"
              alignItems="center">
            <Grid item xs={'auto'} p={1}><h1 className={'title-code color-gradient'}>psudo</h1>
            </Grid>

            <Grid item xs={'auto'} sx={{zIndex: 10000}}>
                <IconButton
                    id="history-button"
                    aria-controls={openHistory ? 'basic-menu' : undefined}
                    aria-haspopup="true"
                    aria-expanded={openHistory ? 'true' : undefined}
                    onClick={handleHistoryClick}
                >
                    <HistoryIcon/>
                </IconButton>
                <Menu
                    id="basic-menu"
                    anchorEl={anchorElHistory}
                    open={openHistory}
                    onClose={handleCloseHistory}
                    MenuListProps={{
                        'aria-labelledby': 'basic-button',
                    }}
                    sx={{marginTop: 5}}
                >
                    <PastPalettes/>

                </Menu>
            </Grid>
        </Grid>

        <Grid container sx={{bottom: 0, left: 0, position: 'absolute', zIndex: 100000}} direction="row"
              justifyContent="flex-start"
              alignItems="center">
            <Grid item xs={'auto'}>
                <Footer/>
            </Grid>
            <Grid item xs={'auto'}>
                <IconButton
                    id="source-button"
                    aria-controls={openSource ? 'basic-menu' : undefined}
                    aria-haspopup="true"
                    aria-expanded={openSource ? 'true' : undefined}
                    onClick={handleOverlapClick}

                >
                    <JoinInnerIcon/>
                </IconButton>
                <Menu
                    id="basic-menu"
                    anchorEl={anchorElOverlap}
                    open={openOverlap}
                    onClose={handleCloseOverlap}
                    MenuListProps={{
                        'aria-labelledby': 'basic-button',
                    }}
                >
                    <Grid
                        container
                        direction="column"
                        justifyContent="center"
                        alignItems="center"
                        p={0}
                        m={0}
                    >
                        <Grid item p={0} m={0}>
                            <h4 style={{margin: 0, padding: 0}}>Channel Overlap</h4>
                        </Grid>

                        <Grid item p={0} m={0}>
                            <SingleChannelWrapper label={'Channel Overlap'} channelIndex={-1}
                                                  width={128} height={128} absoluteScale={1}
                                                  channelsVisible={channelsVisible}>
                            </SingleChannelWrapper>
                        </Grid>
                    </Grid>
                </Menu>
            </Grid>
        </Grid>

        <Grid container sx={{bottom: 0, right: 0, position: 'absolute', zIndex: 100000}} direction="row"
              justifyContent="flex-end"
              alignItems="center" p={1}>
            <Grid item xs={'auto'} id={'built-with'}>
                    <span>Built With<a className={'color-gradient'} href={'http://viv.gehlenborglab.org/'}>&nbsp;Viv</a></span>
            </Grid>
        </Grid>
        <Grid container sx={{top: 0, right: 0, position: 'absolute'}} direction="row"
              justifyContent="flex-end"
              alignItems="center" p={1}>
            <Grid item xs={'auto'} style={{zIndex: 100}}>
                <IconButton
                    id="source-button"
                    aria-controls={openSource ? 'basic-menu' : undefined}
                    aria-haspopup="true"
                    aria-expanded={openSource ? 'true' : undefined}
                    onClick={handleSourceClick}
                    style={{borderRadius: 10}}>
                    <span style={{'fontSize': '1rem'}}>Source&nbsp;</span>
                    <AddPhotoAlternateIcon/>
                </IconButton>
                <Menu
                    id="basic-menu"
                    anchorEl={anchorElSource}
                    open={openSource}
                    onClose={handleCloseSource}
                    MenuListProps={{
                        'aria-labelledby': 'basic-button',
                    }}
                    style={{marginTop: 5, width: 350}}>
                    <Grid container height={60} direction="row"
                          justifyContent="space-around"
                          alignItems="center" style={{width: (showLinkInput || showFileUpload) ? 300 : 150}}>
                        <Grid item xs={'auto'} align="center">
                            <IconButton onClick={handleLinkButtonClick}
                                        color={(showLinkInput) ? 'primary' : 'white'}>
                                <InsertLink/>
                            </IconButton>
                            <Typography variant="subtitle2" align="center"
                                        style={{
                                            transition: 'all 0s linear 0s',
                                            display: (showLinkInput || showFileUpload) ? 'none' : 'block',
                                        }}>
                                Link
                            </Typography>
                        </Grid>
                        <Grid item xs={'auto'} m={0} p={0} align="center" style={{
                            transition: 'width 0s ease-in',
                            width: (showLinkInput || showFileUpload) ? '200px' : '0px',
                        }}>
                            {(showLinkInput || showFileUpload) && (<>
                                {showLinkInput && (<TextField
                                    id="link-input"
                                    variant="outlined"
                                    margin="dense"
                                    size="small"
                                    fullWidth
                                    value={imageUrl}
                                    onChange={updateUrl}
                                    onKeyDown={submitUrl}
                                />)}
                                {showFileUpload && (<DropzoneButton/>)}
                            </>)}
                        </Grid>
                        <Grid item xs={'auto'} align="center">
                            <IconButton onClick={handleFileButtonClick}
                                        color={(showFileUpload) ? 'primary' : 'white'}>
                                <AttachFile/>
                            </IconButton>
                            <Typography variant="subtitle2" align="center"
                                        style={{
                                            transition: 'all 0s linear 0s',
                                            display: (showLinkInput || showFileUpload) ? 'none' : 'block',
                                        }}>
                                File
                            </Typography>
                        </Grid>
                    </Grid>
                </Menu>
            </Grid>
        </Grid>

    </ThemeProvider>)
}

export default PsudoToolbar