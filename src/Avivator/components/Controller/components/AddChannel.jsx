import React, {useCallback} from 'react';
import {Button} from '@mui/material';
import AddIcon from '@mui/icons-material/Add';
import shallow from 'zustand/shallow';
import {useChannelsStore, useImageSettingsStore, useLoader, useViewerStore} from '../../../state';
import {getSingleSelectionStats} from '../../../viewerUtils';

const AddChannel = () => {
    const [
        globalSelection,
        isViewerLoading,
        setIsChannelLoading,
        addIsChannelLoading,
        pyramidResolution
    ] = useViewerStore(
        store => [
            store.globalSelection,
            store.isViewerLoading,
            store.setIsChannelLoading,
            store.addIsChannelLoading,
            store.pyramidResolution
        ],
        shallow
    );
    const [selections, addChannel, setPropertiesForChannel] = useChannelsStore(
        store => [
            store.selections,
            store.addChannel,
            store.setPropertiesForChannel
        ],
        shallow
    );
    const loader = useLoader();
    const {labels} = loader[0];
    const handleChannelAdd = useCallback(() => {
        let selection = Object.fromEntries(labels.map(l => [l, 0]));
        selection = {...selection, ...globalSelection};
        const numSelectionsBeforeAdd = selections.length;
        getSingleSelectionStats({
            loader,
            selection,
            pyramidResolution
        }).then(({domain, contrastLimits}) => {
            console.log('domain', domain, 'contrastLimits', contrastLimits);
            setPropertiesForChannel(numSelectionsBeforeAdd, {
                domains: domain,
                contrastLimits,
                channelsVisible: true
            });
            useImageSettingsStore.setState({
                onViewportLoad: () => {
                    useImageSettingsStore.setState({
                        onViewportLoad: () => {
                        }
                    });
                    setIsChannelLoading(numSelectionsBeforeAdd, false);
                }
            });
            addIsChannelLoading(true);
            addChannel({
                selections: selection,
                ids: String(Math.random()),
                channelsVisible: false
            });
        });
    }, [
        labels,
        loader,
        globalSelection,
        addChannel,
        addIsChannelLoading,
        selections,
        setIsChannelLoading,
        setPropertiesForChannel
    ]);
    return (
        <Button
            // disabled={selections.length === MAX_CHANNELS || isViewerLoading}
            onClick={handleChannelAdd}
            variant="contained"
            // fullWidth
            startIcon={<AddIcon/>}
            size="medium"
        >
            Channel
        </Button>
    );
};
export default AddChannel;
