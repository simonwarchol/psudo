# psudo

See our paper [psudo: Exploring Multi-Channel Biomedical Image Data with Spatially and Perceptually Optimized Pseudocoloring](https://www.biorxiv.org/content/10.1101/2024.04.11.589087v1) for more information.
## Publication

This package is developed following the research conducted by Simon Warchol, Jakob Troidl, Jeremy Muhlich, Robert Krueger, John Hoffer, Tica Lin, Johanna Beyer, Elena Glassman, Peter Sorger, and Hanspeter Pfister. For a detailed explanation of the methodologies and their applications, please refer to the original paper linked above.

### Affiliations
- Harvard John A. Paulson School of Engineering and Applied Sciences
- Harvard Medical School
- New York University Tandon School of Engineering

## Installation

Install Psudo using npm:

```bash
npm i psudo
```

## Usage

To use Psudo in your project, import the required methods from the package:

```javascript
import * as psudo from "psudo";
```
### Methods

Psudo includes the following methods for use in your projects:

- `psudo.channel_gmm`: Function to perform channel-wise Gaussian Mixture Modeling on image data.
- `psudo.calculate_palette_loss`: Calculates the loss in the color palette to optimize image analysis.
- `psudo.ln`: Applies a natural logarithm transformation to data, useful in normalization processes.
- `psudo.optimize`: Optimizes the parameters for image processing based on predefined criteria.

```
