import React, {useContext,} from 'react';
import {AppContext} from "../context/GlobalContext";
import {Backdrop} from "@mui/material";
import RingLoader from "react-spinners/RingLoader";


function LoadingOverlay() {
    const context = useContext(AppContext);
    return (
        <Backdrop sx={{backgroundColor: 'none', zIndex: 100000}} open={context?.isLoading}>
            <RingLoader size={150} color={'#FFFFFF'}/>
        </Backdrop>
    );
}

export default LoadingOverlay