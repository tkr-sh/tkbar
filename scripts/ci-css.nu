let tmp = (mktemp -d)
cp -r ./src/ui/styles $tmp
nu ./scripts/css.nu | ignore
diff -r ($tmp | path join styles) ./src/ui/styles
