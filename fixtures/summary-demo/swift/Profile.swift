struct Profile {
    let name: String
    let nickname: String?
    let age: Int?
    let manager: Profile?

    func displayName() -> String {
        return nickname ?? name
    }

    func ageLabel() -> String {
        guard let age = age else {
            return "unknown"
        }
        return age >= 18 ? "adult" : "minor"
    }

    func managerName() -> String {
        return manager?.nickname ?? manager?.name ?? "nobody"
    }
}
