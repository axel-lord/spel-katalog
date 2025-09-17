# Sample zsh script
while IFS=$'\n' read -r DATA; do
    jq '.' <<< $DATA
done
