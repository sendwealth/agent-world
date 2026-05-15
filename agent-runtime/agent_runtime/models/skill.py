from pydantic import BaseModel, ConfigDict, Field, field_validator


class Skill(BaseModel):
    """Represents a single skill possessed by an agent."""

    model_config = ConfigDict()

    name: str = Field(..., min_length=1, description="Unique skill name")
    max_level: int = Field(default=10, ge=1, description="Maximum achievable level")
    level: int = Field(default=1, ge=1, description="Current skill level")
    experience: int = Field(default=0, ge=0, description="Accumulated experience points")
    next_level_exp: int = Field(default=100, ge=1, description="XP needed for next level")

    @field_validator("level")
    @classmethod
    def level_within_bounds(cls, v, info):
        max_level = info.data.get("max_level", 10) if info.data else 10
        if v > max_level:
            raise ValueError(f"level ({v}) cannot exceed max_level ({max_level})")
        return v

    def add_experience(self, xp: int) -> bool:
        """Add experience points and level up if threshold met.

        Returns True if a level-up occurred.
        """
        if self.level >= self.max_level:
            return False

        self.experience += xp
        leveled_up = False

        while self.level < self.max_level and self.experience >= self.next_level_exp:
            self.experience -= self.next_level_exp
            self.level += 1
            self.next_level_exp = int(self.next_level_exp * 1.5)
            leveled_up = True

        return leveled_up
