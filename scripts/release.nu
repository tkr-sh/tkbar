
def verify_version []: nothing -> string {
    let tkbar_version = (open Cargo.toml | get package | get version)
    let ci_version = (open ./ci/Cargo.toml | get package | get version)
    let flake_version = (cat ./flake.nix | rg -U ' \s*pname = "tkbar";\n\s*version = "([^"]*)";' -r '$1' -N -I)

    if $tkbar_version != $ci_version or $tkbar_version != $flake_version {
        print $"There is a conflict in software version.
This requires manual intervention.

tkbar: `($tkbar_version)`
ci: `($ci_version)`
flake: `($flake_version)`"
        exit 1
    }

    return $tkbar_version;
}


def main [$version:string] {
    let version = ($version | str trim --left --char 'v');
    let previous_version = (verify_version);
    
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
 
    # Tag replacement
    sed -i $"s/($previous_version)/($version)/g" ./Cargo.toml
    sed -i $"s/($previous_version)/($version)/g" ./ci/Cargo.toml
    sed -i $"s/($previous_version)/($version)/g" ./flake.nix

    cargo check

    git diff
    
    print -n "Ok to commit ? [Y/n] "
    let user_input = (input --numchar 1)

    if $user_input == 'n' {
        print -e "Release aborted"
        exit 1
    }

    git add ./Cargo.toml ./ci/Cargo.toml ./flake.nix
    git commit -m $"🏷️ release: v($version)"
    git push origin main
    git tag $version
    git push origin tag $version
    git push main stable
}
