import * as React from "react";
import Button from "@mui/material/Button";
import Dialog from "@mui/material/Dialog";
import DialogActions from "@mui/material/DialogActions";
import DialogContent from "@mui/material/DialogContent";
import DialogContentText from "@mui/material/DialogContentText";
import Link from "@mui/material/Link";
import Box from "@mui/material/Box";
import Avatar from "@mui/material/Avatar";

export default function InfoDialog() {
  const [open, setOpen] = React.useState(true);

  const handleClose = () => {
    setOpen(false);
  };

  return (
    <div>
      <Dialog
        open={open}
        onClose={handleClose}
        aria-labelledby="info-dialog-title"
        aria-describedby="info-dialog-description"
        sx={{ zIndex: 100001 }}
      >
        <DialogContent>
          <Box sx={{ display: "flex", justifyContent: "center", mt: 2 }}>
            <Avatar
              src="/LabsHeader.png"
              variant="square"
              sx={{ width: "100%", height: "auto" }}
            />
          </Box>
          <DialogContentText>
            As detailed in{" "}
            <Link href="https://www.biorxiv.org/content/10.1101/2024.04.11.589087v1">
              Exploring Multi-Channel Biomedical Image Data with Spatially and
              Perceptually Optimized Pseudocoloring
            </Link>
            .
            <br />
            Presented at{" "}
            <Link href="https://event.sdu.dk/eurovis">EuroVis 2024</Link> and to
            appear in{" "}
            <Link href="https://onlinelibrary.wiley.com/journal/14678659">
              Computer Graphics Forum
            </Link>
            .<br />
            <Link href="https://doi.org/10.1101/2024.04.11.589087">
              https://doi.org/10.1101/2024.04.11.589087
            </Link>
          </DialogContentText>
          <Box sx={{ display: "flex", justifyContent: "center", mt: 2 }}>
            <Avatar
              src="/NewTeaser.png"
              variant="square"
              sx={{ width: "100%", height: "auto" }}
            />
          </Box>
          <DialogContentText>
            Authors: <Link href="https://simonwarchol.com">Simon Warchol</Link>
            <sup>1,2</sup>, Jakob Troidl<sup>1</sup>, Jeremy Muhlich<sup>2</sup>
            , Robert Krueger<sup>3</sup>, John Hoffer<sup>2</sup>, Tica Lin
            <sup>1</sup>, Johanna Beyer<sup>1</sup>, Elena Glassman<sup>1</sup>,
            Peter Sorger<sup>2</sup>, Hanspeter Pfister<sup>1</sup>
            <div>
              <sup>1</sup>{" "}
              <i>
                Harvard John A. Paulson School of Engineering and Applied
                Sciences
              </i>
              <br />
              <sup>2</sup> <i>Harvard Medical School</i>
              <br />
              <sup>3</sup>{" "}
              <i>New York University Tandon School of Engineering</i>
            </div>
            <br />
            For more information, visit the{" "}
            <Link href="https://github.com/simonwarchol/psudo">
              GitHub repository
            </Link>{" "}
            or contact the maintainer at{" "}
            <Link href="mailto:simonwarchol@g.harvard.edu">
              simonwarchol@g.harvard.edu
            </Link>
            .
          </DialogContentText>
        </DialogContent>
        <DialogActions>
          <Button onClick={handleClose}>Dismiss</Button>
        </DialogActions>
      </Dialog>
    </div>
  );
}
