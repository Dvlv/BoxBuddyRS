for dir in $(find po -type d)
do
    if [ -d $dir/LC_MESSAGES ]; then
	    fname=$(basename "$dir");
	    ( set -x; msgfmt $dir/LC_MESSAGES/$fname.po -o $dir/LC_MESSAGES/boxbuddyrs.mo);
    fi
done

