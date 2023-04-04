import React, {useContext} from 'react';
import {FormControl, InputLabel, Select} from '@mui/material';
// import {COLORMAP_OPTIONS} from '../../../constants';
import {useImageSettingsStore, useViewerStore} from '../../../state';
import {COLORMAP_OPTIONS} from "../../../../context/GlobalContext.jsx";

function ColormapSelect() {
    const colormap = useImageSettingsStore(store => store.colormap);
    const isViewerLoading = useViewerStore(store => store.isViewerLoading);
    return (<FormControl fullWidth variant="standard" sx={{m: 1}}>
        <InputLabel htmlFor="colormap-select">
            Additive Blending
        </InputLabel>
        <Select
            native
            onChange={e => useImageSettingsStore.setState({colormap: e.target.value})}
            value={colormap}
            inputProps={{
                name: 'colormap', id: 'colormap-select'
            }}
            disabled={isViewerLoading}
        >
            {COLORMAP_OPTIONS.map(name => (<option key={name} value={name}>
                {name}
            </option>))}
        </Select>
    </FormControl>);
}

export default ColormapSelect;
