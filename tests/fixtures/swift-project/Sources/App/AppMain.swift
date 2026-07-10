import NetworkKit
import Models

@main
struct AppMain {
    static func main() {
        let client = APIClient()
        let user = User(name: "test")
        client.fetch(user: user)
    }
}
