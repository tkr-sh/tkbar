def main [$name:string] {
    let file = $"../ressources/bar-($name).png"
    grimshot copy output
    wl-paste o> $file
    magick $file -crop 78x1920+0+0 +repage $file
}
