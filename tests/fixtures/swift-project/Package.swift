// swift-tools-version: 5.9
import PackageDescription

let package = Package(
    name: "TestApp",
    targets: [
        .executableTarget(name: "App", dependencies: ["NetworkKit", "Models"]),
        .target(name: "NetworkKit", dependencies: ["Models"]),
        .target(name: "Models"),
        .testTarget(name: "AppTests", dependencies: ["App"]),
    ]
)
