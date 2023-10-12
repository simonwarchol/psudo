import React, {useEffect} from 'react';

import {useViewerStore} from './state';
import {useImage} from './hooks';
import Viewer from './components/Viewer';
import DropzoneWrapper from './components/DropzoneWrapper';

import './Avivator.css';
import Controller from "./components/Controller";
import SnackBars from "./components/Snackbars";

/**
 * This component serves as batteries-included visualization for OME-compliant tiff or zarr images.
 * This includes color contrastLimits, selectors, and more.
 * @param {Object} props
 * @param {Object} props.history A React router history object to create new urls (optional).
 * @param {Object} args.sources A list of sources for a dropdown menu, like [{ url, description }]
 * */
export default function Avivator(props) {
    const {history, source: initSource, isDemoImage} = props;
    const allowNavigation = props?.allowNavigation || false;
    const isViewerLoading = useViewerStore(store => store.isViewerLoading);
    const source = useViewerStore(store => store.source);
    const [dimensions, setDimensions] = React.useState(null);
    const ref = React.useRef(null);
    const handleResize = () => {
        let dimensions = null;
        if (ref?.current) {
            dimensions = {
                width: ref.current.offsetWidth,
                height: ref.current.offsetHeight
            }
        }
        setDimensions(dimensions);
    }
    useEffect(() => {
        window.addEventListener("resize", handleResize, false);
    }, []);
    useEffect(() => {
        handleResize();
    }, [ref])

    useImage(source, history);
    return (
        <div className={'avivator-wrapper'} style={{width: '100%', height: '100%'}} ref={ref}>
            {ref && dimensions !== null && (
                <>
                    <DropzoneWrapper>{!isViewerLoading &&
                        <Viewer allowNavigation={allowNavigation} dimensions={dimensions}/>}</DropzoneWrapper>
                    {/* <Controller/> */}
                    <SnackBars/>
                    {/*<Footer/>*/}
                </>
            )}
        </div>
    );
}
