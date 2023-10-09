/* eslint-disable no-nested-ternary */
import shallow from 'zustand/shallow';
import React, {useContext, useEffect, useState} from 'react';
import {getDefaultInitialViewState, PictureInPictureViewer} from '@hms-dbmi/viv';
import {useChannelsStore, useImageSettingsStore, useLoader, useViewerStore} from '../state';
import {DEFAULT_OVERVIEW} from '../constants';
import {AppContext} from "../../context/GlobalContext";
import {ColorMixingExtension} from "../../components/shaders/index.js";


const Viewer = (props) => {
        const context = useContext(AppContext);
        const showChannel = props?.showChannel;

        const viewerRef = React.useRef(null);


        const [overlayLayers, setOverlayLayers] = useState([]);


        let [colors, contrastLimits, channelsVisible, selections, pixelValues] =
            useChannelsStore(
                store => [
                    store.colors,
                    store.contrastLimits,
                    store.channelsVisible,
                    store.selections,
                    store.pixelValues
                ],
                shallow
            );
        if (showChannel || showChannel === 0) {
            channelsVisible = channelsVisible.map(() => false);
            channelsVisible[showChannel] = true;
        }
        // console.log('colors', colors, 'contrastLimits', contrastLimits, 'channelsVisible', channelsVisible, 'selections', selections)
        const {width, height} = props.dimensions;
        const allowNavigation = props?.allowNavigation || false;
        const loader = useLoader();
        const [
            lensSelection,
            colormap,
            resolution,
            lensEnabled,
            zoomLock,
            panLock,
            isOverviewOn,
            onViewportLoad,
            useFixedAxis
        ] = useImageSettingsStore(
            store => [
                store.lensSelection,
                store.colormap,
                store.resolution,
                store.lensEnabled,
                store.zoomLock,
                store.panLock,
                store.isOverviewOn,
                store.onViewportLoad,
                store.useFixedAxis
            ],
            shallow
        );

        const svgSize = 512;


        function svgToDataURL(svg) {
            return `data:image/svg+xml;charset=utf-8,${encodeURIComponent(svg)}`;
        }

        const getColorSpace = (colormap) => {
            return [new ColorMixingExtension({colormap})];

        }

        const onClick = () => {
            const _coordinate = useViewerStore.getState()?.coordinate?.map((x) => Math.round(x));
        }


        useEffect(() => {
            const viewState = getDefaultInitialViewState(loader, {height, width}, 0.5);
            useViewerStore.setState({
                viewState: viewState,
                pyramidResolution: Math.min(Math.max(Math.round(-viewState?.zoom), 0), loader.length - 1)
            })

        }, [loader, height, width])

        return (
            <div className={'viewerWrapper'} onClick={onClick}>
                <PictureInPictureViewer
                    loader={loader}
                    contrastLimits={contrastLimits}
                    colors={colors}
                    channelsVisible={channelsVisible}
                    selections={selections}
                    height={height}
                    width={width}
                    zoomLock={false}
                    panLock={false}
                    clickCenter={false}
                    overview={DEFAULT_OVERVIEW}
                    overviewOn={isOverviewOn}
                    hoverHooks={{
                        handleValue: v => useViewerStore.setState({pixelValues: v}),
                        handleCoordinate: v => useViewerStore.setState({coordinate: v}),
                    }}
                    lensSelection={lensSelection}
                    lensEnabled={lensEnabled}
                    extensions={getColorSpace(colormap)}
                    colormap={colormap || 'viridis'}
                    onViewStateChange={({viewState: newViewState, viewId}) =>
                        useViewerStore.setState({
                            viewState: {...newViewState, id: viewId},
                            pyramidResolution: Math.min(Math.max(Math.round(-newViewState?.zoom), 0), loader.length - 1)

                        })
                    }
                    deckProps={{
                        layers: overlayLayers

                    }}
                    onHover={v => {
                        useViewerStore.setState({coordinate: v?.coordinate});
                    }}


                />
            </div>
        );
    }
;
export default Viewer;


// Estimate
