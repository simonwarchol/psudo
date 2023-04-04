import React from 'react';
import {Button} from '@mui/material';
import {useDropzone} from '../../../hooks';

export default function DropzoneButton() {
    const {getRootProps, getInputProps} = useDropzone();

    return (<Button
        fullWidth
        variant="outlined"
        style={{
            borderStyle: 'dashed', backgroundColor: 'transparent', height: '2rem'
        }}
        size="small"
        /* eslint-disable-next-line react/jsx-props-no-spreading */
        {...getRootProps()}
    >
        {/* eslint-disable-next-line react/jsx-props-no-spreading */}
        <input {...getInputProps({accept: '.tif, .tiff'})} />
        File
    </Button>);
}
