import React, {useReducer, useRef} from 'react';
import {IconButton, MenuItem, MenuList, Paper, Popper} from '@mui/material';
import MoreVertIcon from '@mui/icons-material/MoreVert';
import {ClickAwayListener} from '@mui/base';
import {makeStyles} from '@mui/styles';

import ColorPalette from './ColorPalette';

const useStyles = makeStyles(() => ({
    paper: {
        backgroundColor: 'rgba(0, 0, 0, 0.75)'
    }, span: {
        width: '70px', textAlign: 'center', paddingLeft: '2px', paddingRight: '2px'
    }, colors: {
        '&:hover': {
            backgroundColor: 'transparent'
        }, paddingLeft: '2px', paddingRight: '2px'
    }
}));

function ChannelOptions({handleRemoveChannel, handleColorSelect, disabled}) {
    const [open, toggle] = useReducer(v => !v, false);
    const anchorRef = useRef(null);

    const classes = useStyles();
    return (<>
        <IconButton
            aria-label="Remove channel"
            size="small"
            onClick={toggle}
            ref={anchorRef}
            disabled={disabled}
        >
            <MoreVertIcon fontSize="small"/>
        </IconButton>
        <Popper open={open} anchorEl={anchorRef.current} placement="bottom-end">
            <Paper className={classes.paper}>
                <ClickAwayListener onClickAway={toggle}>
                    <MenuList id="channel-options">
                        <MenuItem dense disableGutters onClick={handleRemoveChannel}>
                            <span className={classes.span}>Remove</span>
                        </MenuItem>
                        <MenuItem dense disableGutters className={classes.colors}>
                            <ColorPalette handleColorSelect={handleColorSelect}/>
                        </MenuItem>
                    </MenuList>
                </ClickAwayListener>
            </Paper>
        </Popper>
    </>);
}

export default ChannelOptions;
