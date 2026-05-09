I want you to create a simple voxel engine in rust. It reads a json file of voxels, renders the voxels and save it as a png file.

The CLI looks like this:

```
cargo run -- input.json output.png
```

The voxel json looks like this:

```json
{
    "voxels": [
        # color is rgba, and each value has range 0..=255.
        { "pos": [-1, -1, 0], "color": [192, 32, 32, 255] },
        { "pos": [-1, 0, 0], "color": [192, 32, 32, 255] },
        { "pos": [-1, 1, 0], "color": [192, 32, 32, 255] },
        { "pos": [0, -1, 0], "color": [192, 32, 32, 255] },
        { "pos": [0, 0, 0], "color": [32, 192, 32, 255] },
        { "pos": [0, 1, 0], "color": [192, 32, 32, 255] },
        { "pos": [1, -1, 0], "color": [192, 32, 32, 255] },
        { "pos": [1, 0, 0], "color": [192, 32, 32, 255] },
        { "pos": [1, 1, 0], "color": [192, 32, 32, 255] },
    ],

    # There can be multiple light sources.
    # You also have to implement shadows.
    # Intensity is between 0.1 and 2.0.
    "lights": [
        { "pos": [1, 1, 2], "intensity": 0.5 },
    ],

    # The camera is located at this position.
    # The camera is always looking at (0, 0, 0).
    # Zoom is between 0.1 and 2.0.
    "camera": { "pos": [5, 5, 5], "zoom": 1.0 }
}
```

The position of every object (voxel, light and camera) are in between -30 and 30. That means, `[-30, -30, -30]` and `[30, 30, 30]` are valid positions, but `[31, 31, 31]` is not.
