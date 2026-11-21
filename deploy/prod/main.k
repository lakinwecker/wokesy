import manifests
import wokesy

manifests.yaml_stream(wokesy.resources(wokesy.Release {
    image_version = "0.4.6"
}))
