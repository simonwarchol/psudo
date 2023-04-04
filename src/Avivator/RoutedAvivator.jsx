import React, {useContext} from 'react';
import {useLocation} from 'react-router-dom';
import {createTheme, ThemeProvider} from '@mui/material/styles';
import CssBaseline from '@mui/material/CssBaseline';
import Avivator from './Avivator';
import {AppContext} from "../context/GlobalContext";

const darkTheme = createTheme({
    palette: {
        mode: 'dark',
    },
    props: {
        MuiButtonBase: {
            disableRipple: true
        }
    }
});


// https://reactrouter.com/web/example/query-parameters
function useQuery() {
    return new URLSearchParams(useLocation().search);
}

function RoutedAvivator(props) {
    const query = useQuery();
    const context = useContext(AppContext);
    const allowNavigation = props?.allowNavigation || false;


    // const [pixelValues] =
    //     useChannelsStore(
    //         store => [
    //             store.pixelValues
    //         ],
    //         shallow
    //     );


    return (
        <>
            {context?.randomState?.dataPath !== null && (
                <ThemeProvider theme={darkTheme}>
                    <CssBaseline/>
                    <Avivator history={history} allowNavigation={allowNavigation}/>
                </ThemeProvider>
            )}
        </>
    );
}

export default RoutedAvivator


