def set_tier(member: Member, tier: Literal[1, 2]) -> Literal[True, False]:
    """Move a member to another tier."""
    if member.tier == tier:
        return False
    member.tier = tier
    history.write(None, member.id, tier, 2)
    members.save(member, notify=None)
    return True
