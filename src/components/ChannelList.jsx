import React from 'react';
import {useChannelsStore, useImageSettingsStore, useViewerStore} from "../Avivator/state.js";
import {Checkbox, ListItemText, MenuItem} from "@mui/material";
import shallow from "zustand/shallow";
import _ from "lodash";

function ChannelList() {
    const [channelsVisible] = useChannelsStore(store => [store.channelsVisible], shallow);
    const [channelOptions] = useViewerStore(store => [store.channelOptions], shallow);
    const colormap = useImageSettingsStore(store => store.colormap);
    const isViewerLoading = useViewerStore(store => store.isViewerLoading);

    return (
        <>
            {_.range(6).map((d, i) => {
                return (
                    <MenuItem key={`channel_list_${i}`} m={0} p={0}>
                        <ListItemText primary={`Channel ${i}`}/>
                        <Checkbox checked={channelsVisible[i]} onChange={(e) => {
                            let _tmpChannelsVisible = _.cloneDeep(channelsVisible);
                            _tmpChannelsVisible[i] = !_tmpChannelsVisible[i]
                            useChannelsStore.setState({
                                channelsVisible: _tmpChannelsVisible,
                            });
                        }
                        }/>
                    </MenuItem>
                )
            })
            }
        </>
    );
}

export default ChannelList;
