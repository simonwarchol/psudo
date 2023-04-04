import React, {useEffect, useReducer, useRef, useState} from 'react';
import {Box, Fade, Grid, IconButton, Link, Paper, Popper, Select, TextField, Typography} from '@mui/material';
import InfoIcon from '@mui/icons-material/Info';
import {ClickAwayListener} from '@mui/base';
import {makeStyles} from '@mui/styles';
import DropzoneButton from './DropzoneButton';
import {getNameFromUrl, isMobileOrTablet} from '../../../viewerUtils';
import {useChannelsStore, useViewerStore} from '../../../state';
import CancelIcon from '@mui/icons-material/Cancel';
import SettingsIcon from '@mui/icons-material/Settings';
import './Menu.css';

const useStyles = makeStyles(theme => ({
    root: {
        maxHeight: props => `${props.maxHeight - theme.spacing(4)}px`,
        width: '365px',
        overflowX: 'hidden',
        overflowY: 'scroll',
        '&::-webkit-scrollbar': {
            display: 'none',
            background: 'transparent'
        },
        scrollbarWidth: 'none'
    },
    typography: {
        fontSize: '.8rem'
    },
    paper: {
        paddingBottom: theme.spacing(2),
        paddingRight: theme.spacing(2),
        paddingLeft: theme.spacing(2),
        backgroundColor: "rgba(0, 0, 0, 0.6) !important",
        borderRadius: 2
    },
    item: {
        width: '100%'
    },
    divider: {
        paddingBottom: theme.spacing(1),
        paddingTop: theme.spacing(2)
    }
}));

function Header(props) {
    const image = useChannelsStore(store => store.image);
    const [source, metadata] = useViewerStore(store => [
        store.source,
        store.metadata
    ]);
    const handleSubmitNewUrl = (event, newUrl) => {
        event.preventDefault();
        const newSource = {
            urlOrFile: newUrl,
            // Use the trailing part of the URL (file name, presumably) as the description.
            description: getNameFromUrl(newUrl)
        };
        useViewerStore.setState({source: newSource});
    };
    const onImageSelectionChange = e =>
        useChannelsStore.setState({
            image: e.target.value
        });
    const url = typeof source.urlOrFile === 'string' ? source.urlOrFile : '';
    const [text, setText] = useState(url);
    const [open, toggle] = useReducer(v => !v, false);
    const anchorRef = useRef(null);
    const classes = useStyles(props);

    useEffect(() => setText(url), [url]);

    return (
        <Grid container direction="column" spacing={0}>
            <Grid
                container
                direction="row"
                justifyContent="space-between"
                alignItems="center"
            >

            </Grid>
            {!isMobileOrTablet() && (
                <Grid item xs={12} style={{paddingTop: 16}}>
                    <DropzoneButton/>
                </Grid>
            )}
            {Array.isArray(metadata) && (
                <Grid item xs={12}>
                    <Select native value={image} onChange={onImageSelectionChange}>
                        {metadata.map((meta, i) => (
                            <option key={meta.Name} value={i}>
                                {meta.Name}
                            </option>
                        ))}
                    </Select>
                </Grid>
            )}
        </Grid>
    );
}

function Menu({children, ...props}) {
    const classes = useStyles(props);
    const [isControllerOn, toggleIsControllerOn] = useState(false);

    const toggleShowController = () => {
        toggleIsControllerOn(!isControllerOn);
    }
    return (
        <>
            <Box id={'menu-box'} position="absolute" right={0} top={0} m={1} className={classes.root}
                 sx={{maxHeight: '96%', width: '300px'}}>
                <Grid container direction="column" alignItems="flex-end">
                    <Grid item xs={12}>
                        <IconButton name="details" onClick={toggleShowController}>
                            {(isControllerOn ? <CancelIcon/> : <SettingsIcon className={'settings-icon'}/>)}
                        </IconButton>
                    </Grid>
                    <Grid item xs={12}>
                        <Fade in={isControllerOn}>
                            <Paper className={classes.paper} sx={{height: '100%', overflowY: 'scroll'}}>
                                <Header/>
                                <Grid
                                    container
                                    direction="column"
                                    justifyContent="center"
                                    alignItems="center"
                                >
                                    {children.map((child, i) => {
                                        return (
                                            // eslint-disable-next-line react/no-array-index-key
                                            <Grid item key={i} className={classes.item}>
                                                {child}
                                            </Grid>
                                        );
                                    })}
                                </Grid>
                            </Paper>
                        </Fade>
                    </Grid>
                </Grid>
            </Box>
        </>
    )
}

export default Menu;
