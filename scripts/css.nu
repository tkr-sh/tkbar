let colors = {
    "white": {
        "bg": "#eeeeeeEE",
        "fg": "#222",
        "accent-color": "#444",
    },
    "black": {
        "bg": "#2e2e2eEE",
        "fg": "#eee",
        "accent-color": "#bababa",
    },
    "red": {
        "bg": "#392227EE",
        "fg": "#fde",
        "accent-color": "#ff7790",
    }
};


$colors
| items {|k,v|
    $"@use './src/ui/styles/main.scss' with \(
        $fg: ($v.fg),
        $bg: ($v.bg),
        $accent-color: ($v.accent-color),
    \)" | sass --stdin --style=compressed --no-source-map $"./src/ui/styles/($k).css"
#$k
}

# | values
# | each { $in.1 }
