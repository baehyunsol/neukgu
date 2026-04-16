I'm running out of disk storage, and I want to inspect which file (or directory) is taking such space.

Please write a rust program that, visualizes the disk usage. With this visualization, you can see which files (or directories) are taking space.

It doesn't have to be interactive. It just produces a single output file (it can be html, png, text or whatever you want). And the user can know which files/directories are taking space just by reading the output file.

There can be many scenarios. For example, there can be a single gigantic file hidden deep inside `Documents/` directory. There can be a single directory which has millions of files, and each file is a few kilobytes. There can be a single directory which has hundreds of files, and each file is a few hundred megabytes. ... There are so many cases. The program has to cover all the cases. The program has to pin-point exactly which files/directories are the problem.
