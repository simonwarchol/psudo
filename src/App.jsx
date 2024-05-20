import React from 'react'
import './App.css'
import {Grid} from "@mui/material";
import RoutedAvivator from "./Avivator/RoutedAvivator";
import {ContextWrapper} from "./context/GlobalContext";
import "@fontsource/inter";
import LoadingOverlay from "./components/LoadingOverlay.jsx";
import PsudoToolbar from "./components/PsudoToolbar.jsx";
import IndividualChannelsWrapper from "./Avivator/IndividualChannelsWrapper.jsx";
import {BrowserRouter as Router, Route} from "react-router-dom";
import InfoDialog from "./components/Dialog.jsx";


function App() {


    return (
        <ContextWrapper>
            <InfoDialog/>
            <LoadingOverlay/>
            <div className="App">
                <Router>

                    <Grid container sx={{height: '100%', width: '100%'}}
                          spacing={1}>
                        <Route
                            path="/"
                            render={routeProps => (
                                <Grid container sx={{height: '100%', width: '100%'}}
                                      spacing={1}>
                                    <PsudoToolbar/>
                                    <Grid item xs={12} container justifyContent="center"
                                          direction="row">
                                        <Grid item xs={6} sx={{paddingLeft: 5, paddingTop: 10}}>
                                            <RoutedAvivator allowNavigation={true}/>
                                        </Grid>
                                        <Grid item xs={6} container
                                              justifyContent="space-between"
                                        >
                                            <IndividualChannelsWrapper/>
                                        </Grid>

                                    </Grid>

                                </Grid>

                            )}
                        />

                    </Grid>
                </Router>
            </div>
        </ContextWrapper>
    )
}

export default App
