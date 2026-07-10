@testable import App
import XCTest

class AppTests: XCTestCase {
    func testNavigation() {
        let nav = NavigationManager.shared
        nav.navigate(to: "home")
    }
}
