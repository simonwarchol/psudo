import React, {useContext, useEffect} from 'react';
import Viewer from './components/Viewer';

import './Avivator.css';
import {AppContext} from "../context/GlobalContext.jsx";

export default function MiniAvivator(props) {
    const context = useContext(AppContext);
    const allowNavigation = props?.allowNavigation || false;
    const showChannel = props?.showChannel;
    const [dimensions, setDimensions] = React.useState(null);
    const ref = React.useRef(null);
    const channelsVisible = props?.channelsVisible
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
    }, [ref, channelsVisible])

    return (
        <>
            {context?.randomState?.dataPath !== null && (
                <div className={'avivator-wrapper'} style={{width: '100%', height: '100%'}} ref={ref}>
                    {ref && dimensions !== null &&  (
                        <>
                            <Viewer allowNavigation={allowNavigation} dimensions={dimensions}
                                    showChannel={showChannel}/>
                        </>
                    )}
                </div>
            )}
        </>
    );
}
