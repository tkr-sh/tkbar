let colors = {
    "white": {
        "bg": "#eeeeeeEE",
        "fg": "#222",
        "accent-color": "#777",
    },
    "black": {
        "bg": "#2e2e2eEE",
        "fg": "#eee",
        "accent-color": "#888",
    },
    "red": {
        "bg": "#392227EE",
        "fg": "#fde",
        "accent-color": "#ff7790",
    },
    "orange": {
        "bg": "#392a22ee",
        "fg": "#ffdfdd",
        "accent-color": "#ffa977",
    },
    "yellow": {
        "bg": "#393222ee",
        "fg": "#fff1dd",
        "accent-color": "#ddcc77",
    },
    "green": {
        "bg": "#223925ee",
        "fg": "#e2ffdd",
        "accent-color": "#77ee88",
    },
    "cyan": {
        "bg": "#223839ee",
        "fg": "#ddfff7",
        "accent-color": "#77dde8",
    },
    "blue": {
        "bg": "#222939ee",
        "fg": "#ddf0ff",
        "accent-color": "#7799ff",
    },
    "purple": {
        "bg": "#2f2239ee",
        "fg": "#e6ddff",
        "accent-color": "#c677ff",
    },
    "pink": {
        "bg": "#392231ee",
        "fg": "#ffddfd",
        "accent-color": "#ff77cd",
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
