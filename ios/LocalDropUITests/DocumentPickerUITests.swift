import XCTest

final class DocumentPickerUITests:XCTestCase {
    func testOpenReturnsToAppAndFileIsReadable() {
        continueAfterFailure = false
        let app = XCUIApplication()
        app.launchArguments = ["--file-picker-ui-test"]
        app.launch()
        app.buttons["choose-file"].tap()
        let file = app.cells.matching(NSPredicate(format:"identifier BEGINSWITH %@","LocalDrop-picker-fixture")).firstMatch
        XCTAssertTrue(file.waitForExistence(timeout:30),app.debugDescription)
        file.tap()
        let open = app.buttons["Open"].firstMatch
        if open.waitForExistence(timeout:3) { XCTAssertTrue(open.isEnabled); open.tap() }
        let result = app.staticTexts["picker-result"]
        let expected = NSPredicate(format:"label == %@","Imported 1: picker fixture bytes")
        expectation(for:expected,evaluatedWith:result)
        waitForExpectations(timeout:15)
    }
}
