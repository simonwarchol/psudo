import { LayerExtension } from "@deck.gl/core";
import { getDefaultPalette, padColors } from "../utils";
import srgbFs from "../fragment-shaders/rgb-color-mixing-fs.glsl";
import oklabFs from "../fragment-shaders/oklab-color-mixing-fs.glsl";

import _ from "lodash";
import { useContext } from "react";
import { AppContext } from "../../../context/GlobalContext.jsx";
import { useImageSettingsStore } from "../../../Avivator/state.js";

const defaultProps = {
  colors: { type: "array", value: null, compare: true },
  lensEnabled: { type: "boolean", value: false, compare: true },
  lensSelection: { type: "number", value: 1, compare: true },
  lensRadius: { type: "number", value: 100, compare: true },
  lensOpacity: { type: "number", value: 1.0, compare: true },
  overlapView: { type: "boolean", value: false, compare: true },
  lensBorderColor: { type: "array", value: [255, 255, 255], compare: true },
  lensBorderRadius: { type: "number", value: 0.02, compare: true },
};

const ColorMixingExtension = class extends LayerExtension {
  constructor(props) {
    super(props);
  }

  getShaders() {
    const self = this;
    const getFs = (colormap) => {
      switch (colormap) {
        case "sRGB":
          return srgbFs;
        case "Oklab":
          return oklabFs;

        // case 'Oklab Gamut Clip Preserve Chroma':
        //     return oklab_gamut_clip_preserve_chroma;
        // case 'Oklab Gamut Clip Adaptive L_0 = 0.5':
        //     return oklab_gamut_clip_adaptive_L0_0_5_FS;
        // case 'Oklab Gamut Clip Adaptive L_0 = L_cusp':
        //     return oklab_gamut_clip_adaptive_L0_L_cusp;
        // case 'Oklab Gamut Clip Project To 0.5':
        //     return oklab_gamut_clip_project_to_0_5;
        // case 'Oklab Gamut Clip Project To L_cusp':
        //     return oklab_gamut_clip_project_to_L_cusp;
        default:
          return oklabFs;
      }
    };

    // For some reason this is called twice for the base class and parent class,
    // so we need to check if the parent class has the colormap property
    const colormap = self?.opts?.colormap || self?.props?.colormap;
    // console.log('getShadersSelf', self, colormap);
    const fragmentShader = getFs(colormap);
    useImageSettingsStore.setState({ fragmentShader, colormap });
    return {
      ...super.getShaders(),
      modules: [
        {
          name: "color-mixing-module",
          fs: fragmentShader,
          inject: {
            "fs:DECKGL_MUTATE_COLOR": `
                       vec3 rgb = rgba.rgb;
                       mutate_color(rgb, intensity0, intensity1, intensity2, intensity3, intensity4, intensity5, vTexCoord);
                       rgba = vec4(rgb, 1.);
                      `,
          },
        },
      ],
    };
  }

  draw() {
    const { colors, channelsVisible } = this.props;
    const channels = channelsVisible || this.selections.map(() => true);
    const paddedColors = padColors({
      channelsVisible: channels,
      colors: colors || getDefaultPalette(this.props.selections.length),
    });
    const uniforms = {
      colors: paddedColors,
      channelsVisible: _.size(_.filter(channels)),
    };
    // eslint-disable-next-line no-unused-expressions
    this.state.model?.setUniforms(uniforms);
  }

  updateState({ props, oldProps, changeFlags, ...rest }) {
    super.updateState({ props, oldProps, changeFlags, ...rest });
    if (props.colormap !== oldProps.colormap) {
      const { gl } = this.context;
      if (this.state.model) {
        this.state.model.delete();
        this.setState({ model: this._getModel(gl) });
      }
    }
  }
};

ColorMixingExtension.extensionName = "ColorMixingExtension";
ColorMixingExtension.defaultProps = defaultProps;

export default ColorMixingExtension;
