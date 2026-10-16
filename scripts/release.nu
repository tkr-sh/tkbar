
def current_version []: nothing -> string {
    return (open ./Cargo.toml | get package | get version)
}


def main [$version:string] {
    let version = ($version | str trim --left --char 'v');
    let previous_version = (current_version);
    
    print -n $"Releasing version `(ansi yellow)($previous_version)(ansi default)` => `(ansi green)($version)(ansi default)` ? [y/N] "
    let user_input = (input --numchar 1)

    if $user_input != 'y' {
        print -e "Release aborted"
        exit 1
    }

    if not (git status | str contains 'nothing to commit') {
        git status
        print -e "Dirty"
        exit 1
    }
 
    # Tag replacement (Cargo.toml only; everything else derives from it)
    sed -i $"s/($previous_version)/($version)/g" ./Cargo.toml

    cargo check

    git diff
    
    print -n "Ok to commit ? [Y/n] "
    let user_input = (input --numchar 1)

    if $user_input == 'n' {
        print -e "Release aborted"
        exit 1
    }

    git add ./Cargo.toml ./Cargo.lock
    git commit -m $"🏷️ release: v($version)"
    git push origin main
    git tag $version
    git push origin tag $version
    git push main stable
}
