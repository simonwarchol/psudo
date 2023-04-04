import React from 'react';
import {Box, Paper, Typography} from '@mui/material';
import shallow from 'zustand/shallow';
import {makeStyles} from '@mui/styles';

import {useLoader, useViewerStore} from '../state';

const useStyles = makeStyles(theme => ({
    typography: {
        fontSize: '.8rem'
    },
    paper: {
        paddingRight: theme.spacing(1),
        paddingLeft: theme.spacing(1),
        backgroundColor: 'rgba(0, 0, 0, 0.75)',
        borderRadius: 2
    }
}));

function formatResolutionStatus(current, total, shape) {
    return `${current}/${total} [${shape.join(', ')}]`;
}

export default function Footer() {
    const classes = useStyles();
    const [pyramidResolution] = useViewerStore(
        store => [store.pyramidResolution],
        shallow
    );
    const loader = useLoader();

    const resolution = pyramidResolution;
    const level = loader[resolution];

    if (!level) return null;
    return (
        <Box
            style={{
                position: 'fixed',
                marginTop: 'calc(5% + 60px)',
                bottom: 0
            }}
        >
            <Paper className={classes.paper}>
                <Typography className={classes.typography}>
                    {formatResolutionStatus(resolution + 1, loader.length, level.shape)}
                </Typography>
            </Paper>
        </Box>
    );
}
