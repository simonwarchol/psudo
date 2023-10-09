import {useContext, useEffect} from 'react';
import {useDropzone as useReactDropzone} from 'react-dropzone';
import shallow from 'zustand/shallow';
// eslint-disable-next-line camelcase
import {unstable_batchedUpdates} from 'react-dom';

import {useChannelsStore, useImageSettingsStore, useLoader, useMetadata, useViewerStore} from './state';
import {buildDefaultSelection, createLoader, getMultiSelectionStats, guessRgb, isInterleaved} from './viewerUtils';
import {FILL_PIXEL_VALUE} from './constants';
import {AppContext} from "../context/GlobalContext.jsx";

export const useImage = (source, history) => {
    const [toggleIsOffsetsSnackbarOn] = useViewerStore(
        store => [store.toggleIsOffsetsSnackbarOn],
        shallow
    );
    const [lensEnabled, toggleLensEnabled, colormap] = useImageSettingsStore(
        store => [store.lensEnabled, store.toggleLensEnabled, store.toggleLensEnabled],
        shallow
    );
    const loader = useLoader();
    const metadata = useMetadata();
    const context = useContext(AppContext);

    useEffect(() => {

    }, [colormap])


    useEffect(() => {
        async function changeLoader() {
            // Placeholder
            useViewerStore.setState({isChannelLoading: [true]});
            useViewerStore.setState({isViewerLoading: true});
            const {urlOrFile} = source;
            const newLoader = await createLoader(
                urlOrFile,
                toggleIsOffsetsSnackbarOn,
                message =>
                    useViewerStore.setState({
                        loaderErrorSnackbar: {on: true, message}
                    })
            );
            let nextMeta;
            let nextLoader;
            if (Array.isArray(newLoader)) {
                if (newLoader.length > 1) {
                    nextMeta = newLoader.map(l => l.metadata);
                    nextLoader = newLoader.map(l => l.data);
                } else {
                    nextMeta = newLoader[0].metadata;
                    nextLoader = newLoader[0].data;
                }
            } else {
                nextMeta = newLoader.metadata;
                nextLoader = newLoader.data;
            }
            if (nextLoader) {
                console.info(
                    'Metadata (in JSON-like form) for current file being viewed: ',
                    nextMeta
                );
                unstable_batchedUpdates(() => {
                    useChannelsStore.setState({loader: nextLoader});
                    useViewerStore.setState({
                        metadata: nextMeta
                    });
                });
            }
        }
        console.log('source: ', source)

        if (source) changeLoader();
    }, [source, history]); // eslint-disable-line react-hooks/exhaustive-deps
    useEffect(() => {
        const changeSettings = async () => {
            // Placeholder
            useViewerStore.setState({isChannelLoading: [true]});
            useViewerStore.setState({isViewerLoading: true});
            const newSelections = buildDefaultSelection(loader[0]);
            const {Channels} = metadata.Pixels;
            console.log('Channels: ', Channels)
            let channelOptions = Channels.map((c, i) => c.Name ?? `Channel ${i}`);
            if (channelOptions?.length === 40) {
                channelOptions = ['DNA0', 'HHLA2', 'CMA1', 'SOX10', 'DNA1', 'S100B', 'KERATIN', 'CD1A', 'DNA2', 'CD163', 'CD3D', 'C8A', 'DNA3', 'MITF', 'FOXP3', 'PDL1', 'DNA4', 'KI67', 'LAG3', 'TIM3', 'DNA5', 'PCNA', 'pSTAT1', 'cPARP', 'DNA6', 'SNAIL', 'aSMA', 'HLADPB1', 'DNA8', 'S100A', 'CD11C', 'PD1', 'DNA9', 'LDH', 'PANCK', 'CCNA2', 'DNA10', 'CCND1', 'CD63', 'CD31']
            } else if (channelOptions?.length === 12) {
                channelOptions = ['DNA_6', 'ELANE', 'CD57', 'CD45', 'DNA_7', 'CD11B', 'SMA', 'CD16', 'DNA_8', 'ECAD', 'FOXP3', 'NCAM']
            }
            // Default RGB.
            let newContrastLimits = [];
            let newDomains = [];
            let newColors = [];
            const isRgb = guessRgb(metadata);
            if (isRgb) {
                if (isInterleaved(loader[0].shape)) {
                    // These don't matter because the data is interleaved.
                    newContrastLimits = [[0, 255]];
                    newDomains = [[0, 255]];
                    newColors = [[255, 0, 0]];
                } else {
                    newContrastLimits = [
                        [0, 255],
                        [0, 255],
                        [0, 255]
                    ];
                    newDomains = [
                        [0, 255],
                        [0, 255],
                        [0, 255]
                    ];
                    newColors = [
                        [255, 0, 0],
                        [0, 255, 0],
                        [0, 0, 255]
                    ];
                }
                if (lensEnabled) {
                    toggleLensEnabled();
                }
                useViewerStore.setState({useColormap: false});
            } else {
                const stats = await getMultiSelectionStats({
                    loader,
                    selections: newSelections
                });

                newDomains = stats.domains;
                newContrastLimits = stats.contrastLimits;
                newColors = [[0, 0, 255], [255, 0, 0], [0, 255, 0], [255, 255, 0], [255, 0, 255], [0, 255, 255]];
                useViewerStore.setState({
                    useColormap: true
                });
            }
            useChannelsStore.setState({
                ids: newDomains.map(() => String(Math.random())),
                selections: newSelections,
                domains: newDomains,
                contrastLimits: newContrastLimits,
                colors: newColors,
                prevColors: newColors,
                channelsVisible: channelOptions.map((d, i) => {
                    return i < 3;
                })
            });

            useViewerStore.setState({
                isChannelLoading: newSelections.map(i => !i),
                isViewerLoading: false,
                pixelValues: new Array(newSelections.length).fill(FILL_PIXEL_VALUE),
                // Set the global selections (needed for the UI). All selections have the same global selection.
                globalSelection: newSelections[0],
                channelOptions
            });
        };

        if (metadata) changeSettings();
    }, [loader, metadata]); // eslint-disable-line react-hooks/exhaustive-deps
};

export const useDropzone = () => {
    const handleSubmitFile = files => {
        let newSource;
        if (files.length === 1) {
            newSource = {
                urlOrFile: files[0],
                // Use the trailing part of the URL (file name, presumably) as the description.
                description: files[0].name
            };
        } else {
            newSource = {
                urlOrFile: files,
                description: 'data.zarr'
            };
        }
        useViewerStore.setState({source: newSource});
    };
    return useReactDropzone({
        onDrop: handleSubmitFile
    });
};
