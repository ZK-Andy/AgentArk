import Foundation
#if canImport(UserNotifications)
import UserNotifications
#endif

public struct DefaultCommandHandler: CompanionCommandHandler {
    private let supportedCapabilities: Set<String>

    public init(supportedCapabilities: [String]) {
        self.supportedCapabilities = Set(supportedCapabilities)
    }

    public func handle(command: CompanionCommand) async -> CompanionCommandResult {
        guard supportedCapabilities.contains(command.capability) else {
            return CompanionCommandResult(
                success: false,
                error: "Unsupported capability."
            )
        }

        switch command.capability {
        case "approval_prompt", "notifications":
            return await CompanionLocalNotifications.post(
                title: notificationTitle(for: command),
                body: notificationBody(for: command)
            )
        default:
            return CompanionCommandResult(
                success: false,
                error: "No local adapter is installed for \(command.capability)."
            )
        }
    }

    private func notificationTitle(for command: CompanionCommand) -> String {
        if let title = command.arguments?.objectString("title"), !title.isEmpty {
            return title
        }
        return command.capability == "approval_prompt" ? "AgentArk Approval" : "AgentArk"
    }

    private func notificationBody(for command: CompanionCommand) -> String {
        for key in ["body", "message", "text"] {
            if let value = command.arguments?.objectString(key), !value.isEmpty {
                return value
            }
        }
        return command.action.isEmpty ? "AgentArk companion notification" : command.action
    }
}

enum CompanionLocalNotifications {
    static func requestAuthorizationIfAvailable() async -> Bool {
        #if canImport(UserNotifications)
        do {
            return try await requestAuthorization()
        } catch {
            return false
        }
        #else
        return true
        #endif
    }

    static func post(title: String, body: String) async -> CompanionCommandResult {
        #if canImport(UserNotifications)
        do {
            let granted = try await requestAuthorization()
            guard granted else {
                return CompanionCommandResult(
                    success: false,
                    error: "Notification permission is not granted."
                )
            }
            try await addNotification(title: title, body: body)
            return CompanionCommandResult(success: true, preview: "Notification shown: \(title).")
        } catch {
            return CompanionCommandResult(success: false, error: error.localizedDescription)
        }
        #else
        print("AgentArk companion notification: \(title) - \(body)")
        return CompanionCommandResult(success: true, preview: "Notification delivered to console.")
        #endif
    }

    #if canImport(UserNotifications)
    private static func requestAuthorization() async throws -> Bool {
        try await withCheckedThrowingContinuation { (continuation: CheckedContinuation<Bool, Error>) in
            UNUserNotificationCenter.current().requestAuthorization(
                options: [.alert, .sound, .badge]
            ) { granted, error in
                if let error {
                    continuation.resume(throwing: error)
                } else {
                    continuation.resume(returning: granted)
                }
            }
        }
    }

    private static func addNotification(title: String, body: String) async throws {
        let content = UNMutableNotificationContent()
        content.title = title
        content.body = body
        content.sound = .default
        let request = UNNotificationRequest(
            identifier: UUID().uuidString,
            content: content,
            trigger: nil
        )
        try await withCheckedThrowingContinuation { (continuation: CheckedContinuation<Void, Error>) in
            UNUserNotificationCenter.current().add(request) { error in
                if let error {
                    continuation.resume(throwing: error)
                } else {
                    continuation.resume(returning: ())
                }
            }
        }
    }
    #endif
}

private extension JSONValue {
    func objectString(_ key: String) -> String? {
        guard case .object(let object) = self else {
            return nil
        }
        guard let raw = object[key], case .string(let value) = raw else {
            return nil
        }
        let trimmed = value.trimmingCharacters(in: .whitespacesAndNewlines)
        return trimmed.isEmpty ? nil : trimmed
    }
}
