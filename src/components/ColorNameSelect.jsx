import React, { useContext, useEffect, useState } from "react";
import { AppContext } from "../context/GlobalContext";
import {
  Checkbox,
  InputLabel,
  ListItemText,
  MenuItem,
  Select,
} from "@mui/material";
import FormControl from "@mui/material/FormControl";
import { v1 as uuidv1 } from "uuid";
import _ from "lodash";
import "./ColorNameSelect.css";

function ColorNameSelect(props) {
  const context = useContext(AppContext);
  const id = uuidv1();
  const multiSelect = props?.multiSelect || false;
  // useRef
  const ref = React.useRef(null);
  const channelIndex = props?.channelIndex;
  const label = props?.label;
  const width = props?.width || multiSelect ? "100%" : "100px";
  const colorNames = [
    "",
    "brown",
    "orange",
    "yellow",
    "black",
    "red",
    "green",
    "blue",
    "pink",
    "grey",
    "purple",
    "darkgreen",
    "navyblue",
    "olive",
    "peach",
    "tan",
    "lightblue",
    "teal",
    "salmon",
    "maroon",
    "white",
    // 'lightgreen', 'darkpurple', 'magenta', 'lavender', 'darkblue', 'beige', 'limegreen', 'cyan', 'skyblue', 'forestgreen', 'darkgrey', 'lightpink', 'darkbrown', 'lightbrown', 'hotpink', 'mauve', 'lightpurple', 'yellowgreen', 'mustard', 'brightgreen', 'darkred', 'offwhite', 'turquoise', 'palegreen', 'burntorange', 'paleyellow', 'lightyellow', 'greygreen', 'cream', 'greyblue', 'lilac', 'bluegrey', 'fuchsia', 'aqua', 'lightorange', 'lightgrey', 'darkteal', 'neongreen', 'gold', 'burgundy', 'periwinkle', 'paleblue', 'royalblue', 'seafoamgreen', 'mint', 'redorange', 'rose', 'darkpink', 'palepink', 'redbrown', 'lime', 'brickred', 'puke', 'violet', 'bluegreen', 'yelloworange', 'plum', 'khaki', 'babyblue', 'darkorange', 'goldenrod', 'flesh', 'greenyellow', 'armygreen', 'taupe', 'sand', 'coral', 'deeppurple', 'rust', 'mustardyellow', 'seagreen', 'brightred', 'brightpurple', 'lightred', 'darkyellow', 'midnightblue', 'brightblue', 'royalpurple', 'indigo', 'crimson', 'orangered', 'huntergreen', 'brightpink', 'chartreuse', 'orangebrown', 'eggplant', 'dustyrose', 'skin', 'slate', 'pastelgreen', 'pinkred', 'sage', 'aquamarine', 'slateblue', 'peagreen', 'bluepurple', 'terracotta', 'brick', 'grassgreen', 'yellowbrown', 'ochre', 'burntsienna', 'steelblue', 'brightyellow', 'bloodred', 'darkturquoise', 'wine', 'kellygreen', 'purpleblue', 'pinkpurple', 'deepblue', 'darkmagenta', 'palepurple', 'deepred', 'springgreen', 'puce', 'mossgreen', 'robinseggblue', 'greenblue', 'neonpink', 'seablue', 'cerulean', 'neonpurple', 'leafgreen', 'blueviolet', 'electricblue', 'cornflowerblue', 'mediumblue', 'oceanblue', 'azure', 'applegreen']
  ];
  let initialVal = [colorNames[0]];
  if (!multiSelect) {
    initialVal = [context.channelColorNames?.[channelIndex]] || initialVal;
  }
  const [selectedColorNames, setSelectedColorNames] = useState(
    initialVal.flat()
  );

  const handleChange = (event) => {
    const {
      target: { value },
    } = event;
    setSelectedColorNames(
      // On autofill we get a stringified value.
      typeof value === "string" ? value.split(",") : value
    );
  };
  useEffect(() => {
    console.log("scn", selectedColorNames);
  }, [selectedColorNames]);

  useEffect(() => {
    if (multiSelect) {
      context.setColorExcluded(selectedColorNames);
    } else {
      if (selectedColorNames?.[0]) {
        let tmpColorNames = _.cloneDeep(context.channelColorNames);
        tmpColorNames[channelIndex] = selectedColorNames;
        context.setChannelColorNames(tmpColorNames);
      }
    }
  }, [selectedColorNames]);

  const clearColorNames = () => {
    setSelectedColorNames([]);
  };

  return (
    <div>
      <FormControl
        sx={{ width: width, marginTop: "10px", marginBottom: "5px" }}
        ref={ref}
      >
        <InputLabel id="exclude-select-label">{label}</InputLabel>
        <Select
          id={`${id}`}
          labelId="exclude-select-label"
          multiple={multiSelect}
          value={selectedColorNames}
          onChange={handleChange}
          defaultValue={""}
          renderValue={(selected) => selected.join(", ")}
          size={"small"}
          className={"color-name-select"}
          height={"100%"}
        >
          {colorNames.map((name) =>
            // {if multi select, then add checkbox, else add nothing}
            multiSelect ? (
              <MenuItem key={name} value={name}>
                <Checkbox checked={selectedColorNames.indexOf(name) > -1} />
                <ListItemText primary={name} />
              </MenuItem>
            ) : (
              <MenuItem key={name} value={name}>
                {name}
              </MenuItem>
            )
          )}
        </Select>
      </FormControl>
    </div>
  );
}

export default ColorNameSelect;
